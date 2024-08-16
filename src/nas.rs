use std::collections::BTreeMap;
use std::io::Split;
use float_cmp::approx_eq;
use float_cmp::ApproxEq;
use float_cmp::F64Margin;
use geo::Area;
use geo::CoordsIter;
use geo::Relate;
use polylabel_mini::LineString;
use polylabel_mini::Point;
use polylabel_mini::Polygon;
use quadtree_f32::Item;
use quadtree_f32::ItemId;
use quadtree_f32::QuadTree;
use quadtree_f32::Rect;
use serde_derive::{Serialize, Deserialize};
use web_sys::console::log_1;
use crate::csv::CsvDataType;
use crate::csv::Status;
use crate::search::NutzungsArt;
use crate::ui::dist_to_segment;
use crate::ui::Aenderungen;
use crate::xlsx::FlstIdParsed;
use crate::xml::XmlNode;
use crate::xml::get_all_nodes_in_subtree;
use proj4rs::Proj;

pub const LATLON_STRING: &str = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NasXMLFile {
    pub ebenen: BTreeMap<String, Vec<TaggedPolygon>>,
    pub crs: String,
}

impl NasXMLFile {

    pub fn create_quadtree(&self) -> NasXmlQuadTree {

        let mut ebenen_map = BTreeMap::new();
        let mut items = BTreeMap::new();
        let mut itemid = 0;
        for (flst_id, polys) in self.ebenen.iter() {
            for (i, p) in polys.iter().enumerate() {
                let id = ItemId(itemid);
                itemid += 1;
                items.insert(id, Item::Rect(p.get_rect()));
                ebenen_map.insert(id, (flst_id.clone(), i));
            }
        }
        
        let qt = QuadTree::new(items.into_iter());

        NasXmlQuadTree {
            items: itemid + 1,
            original: self.clone(),
            qt: qt,
            ebenen_map,
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Label {
    pub lon: f64,
    pub lat: f64,
    pub content: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct GebaeudeInfo {
    pub flst_id: String,
    pub deleted: bool,
    pub gebaeude_id: String,
    pub poly: TaggedPolygon,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct GebaeudeDebugMap {
    pub anzahl_flurstuecke: usize,
    pub anzahl_gebaeude: usize,
    pub aenderungen: Aenderungen,
}

impl NasXMLFile {

    // Returns GeoJSON for all available AX_Gebaeude
    pub fn get_gebaeude(&self, csv: &CsvDataType, aenderungen: &Aenderungen) -> String {

        use quadtree_f32::ItemId;

        let ax_flurstuecke = match self.ebenen.get("AX_Flurstueck") {
            Some(o) => o,
            None => return format!("keine Ebene AX_Flurstueck vorhanden"),
        };

        // Flurstueck_ID => Flurstueck Poly
        let ax_flurstuecke_map = ax_flurstuecke.iter().filter_map(|tp| {
            let flst_id = tp.attributes.get("flurstueckskennzeichen").cloned()?;
            let flst = crate::csv::search_for_flst_id(&csv, &flst_id)?;
            if (flst.1.iter().any(|c| c.status != Status::Bleibt)) {
                let [[min_y, min_x], [max_y, max_x]] = tp.get_fit_bounds();
                let bounds = Rect {
                    max_x: max_x,
                    max_y: max_y,
                    min_x: min_x,
                    min_y: min_y,
                };
                Some((flst_id, bounds))
            } else {
                None 
            }
        }).collect::<BTreeMap<_, _>>();

        let ax_gebaeude = match self.ebenen.get("AX_Gebaeude") {
            Some(o) => o,
            None => return format!("keine Ebene AX_Gebaeude vorhanden"),
        };

        let ax_gebaeude_map = ax_gebaeude.iter().enumerate().filter_map(|(i, tp)| {
            let gebaeude_id = tp.attributes.get("id").cloned()?;
            let item_id = ItemId(i);
            let bounds = tp.get_rect();
            Some((item_id, (gebaeude_id, bounds, tp.clone())))
        }).collect::<BTreeMap<_, _>>();

        // Get intersection of all gebaeude
        let buildings_qt = QuadTree::new(ax_gebaeude_map.iter().map(|(k, v)| {
            (k.clone(), Item::Rect(v.1.clone()))
        }));

        // All buildings witin the given Flst
        let gebaeude_avail = ax_flurstuecke_map
        .iter()
        .flat_map(|(flst_id, flst_rect)| {
            buildings_qt.get_ids_that_overlap(&flst_rect).iter().filter_map(|building_itemid| {
                let building = ax_gebaeude_map.get(&building_itemid)?;
                let already_deleted = aenderungen.gebaeude_loeschen.values().any(|s| s == &building.0);
                Some((building.0.clone(), GebaeudeInfo {
                    flst_id: flst_id.clone(),
                    deleted: already_deleted,
                    gebaeude_id: building.0.clone(),
                    poly: building.2.clone(),
                }))
            }).collect::<Vec<_>>().into_iter()
        })
        .collect::<BTreeMap<_, _>>();
    
        let geom = gebaeude_avail.iter().filter_map(|(k, v)| {

            let holes = v.poly.poly.inner_rings.iter()
            .map(convert_svgline_to_string)
            .collect::<Vec<_>>()
            .join(",");

            let mut attrs = v.poly.attributes.clone();

            attrs.insert("gebaeude_flst_id".to_string(), v.flst_id.clone());
            attrs.insert("gebaeude_geloescht".to_string(), v.deleted.to_string());
            attrs.insert("gebaeude_id".to_string(), v.gebaeude_id.to_string());

            let feature_map = attrs
            .iter().map(|(k, v)| format!("{k:?}: {v:?}"))
            .collect::<Vec<_>>().join(",");

            if v.poly.poly.outer_rings.len() > 1 {
                let polygons = v.poly.poly.outer_rings.iter().map(|p| convert_poly_to_string(&p, &holes)).collect::<Vec<_>>().join(",");
                Some(format!(
                    "{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"MultiPolygon\", \"coordinates\": [{polygons}] }} }}"))
            } else if let Some(p) = v.poly.poly.outer_rings.iter().next() {
                let poly = convert_poly_to_string(p, &holes);
                Some(format!(
                    "{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"Polygon\", \"coordinates\": {poly} }} }}"))
            } else {
                None
            }
        }).collect::<Vec<_>>().join(",");

        format!("{{ \"type\": \"FeatureCollection\", \"features\": [{geom}] }}")

        // serde_json::to_string(&gebaeude_avail).unwrap_or_default()
    }

    pub fn get_geojson_labels(&self, layer: &str) -> Vec<Label> {

        let objekte = match self.ebenen.get(layer) {
            Some(o) => o,
            None => return Vec::new(),
        };

        let mut labels = Vec::new();
        for o in objekte.iter() {

            let flst = o.attributes
            .get("flurstueckskennzeichen")
            .and_then(|s| FlstIdParsed::from_str(s).parse_num());

            let text = match flst {
                Some(s) => s,
                None => continue,
            };

            let label_pos = o.poly.get_label_pos(0.001);

            let label = Label {
                lon: label_pos.x, 
                lat: label_pos.y,
                content: text.format_str(),
                id: o.attributes.get("flurstueckskennzeichen").cloned().unwrap_or_default(),
            };
            labels.push(label);
        }

        labels
    }

    /// Returns GeoJSON für die Ebene
    pub fn get_geojson_ebene(&self, layer: &str) -> String {

        let objekte = match self.ebenen.get(layer) {
            Some(o) => o,
            None => return format!("keine Ebene {layer} vorhanden"),
        };
        tagged_polys_to_featurecollection(&objekte)
    }
}


pub fn tagged_polys_to_featurecollection(objekte: &[TaggedPolygon]) -> String {

    let geom = objekte.iter().filter_map(|poly| {

        let holes = poly.poly.inner_rings.iter()
        .map(convert_svgline_to_string)
        .collect::<Vec<_>>()
        .join(",");

        let feature_map = poly.attributes
        .iter().map(|(k, v)| format!("{k:?}: {v:?}"))
        .collect::<Vec<_>>().join(",");

        if poly.poly.outer_rings.len() > 1 {
            let polygons = poly.poly.outer_rings.iter().map(|p| convert_poly_to_string(&p, &holes)).collect::<Vec<_>>().join(",");
            Some(format!(
                "{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"MultiPolygon\", \"coordinates\": [{polygons}] }} }}"))
        } else if let Some(p) = poly.poly.outer_rings.iter().next() {
            let poly = convert_poly_to_string(p, &holes);
            Some(format!(
                "{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"Polygon\", \"coordinates\": {poly} }} }}"))
        } else {
            None
        }
    }).collect::<Vec<_>>().join(",");

    format!("{{ \"type\": \"FeatureCollection\", \"features\": [{geom}] }}")
}

fn convert_poly_to_string(p: &SvgLine, holes:&str) -> String {
    format!(
        "[{src}{comma}{holes}]", 
        src = convert_svgline_to_string(p),
        comma = if holes.trim().is_empty() { "" } else { "," },
        holes = holes,
    )
}

fn convert_svgline_to_string(q: &SvgLine) -> String {
    format!("[{}]", q.points.iter().map(|s| format!("[{}, {}]", s.x, s.y)).collect::<Vec<_>>().join(","))
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TaggedPolygon {
    pub poly: SvgPolygon,
    pub attributes: BTreeMap<String, String>,
}

impl TaggedPolygon {

    fn check_line_for_points(
        l: &SvgLine, 
        start: &SvgPoint, 
        end: &SvgPoint, 
        log: &mut Vec<String>,
        dst: f64,
    ) -> Vec<SvgPoint> {

        let start = start.round_to_3dec();
        let end = end.round_to_3dec();

        let mut start_is_on_line = None;
        let mut end_is_on_line = None;
        let mut pos_start_extra = None;
        let mut pos_end_extra = None;

        let mut pos_start = match l.points.iter().position(|p| p.equals(&start)) {
            Some(s) => {
                pos_start_extra = Some((s, l.points[s]));
                s
            },
            None => {
                
                let starting_point_on_lines = 
                l.points.iter().enumerate().zip(l.points.iter().skip(1))
                .map(|((pos, s0), e0)| {
                    (pos, s0.clone(), e0.clone(), crate::ui::dist_to_segment(start, *s0, *e0))
                }).collect::<Vec<_>>();

                let nearest_line = starting_point_on_lines
                    .into_iter()
                    .min_by_key(|f| (f.3.distance * 100000.0).round().abs() as usize)
                    .map(|s| s.clone());
                
                let nearest_line = match nearest_line {
                    Some(s) => s,
                    None => return Vec::new(),
                };

                if nearest_line.3.distance > dst {
                    return Vec::new();
                }

                start_is_on_line = Some(nearest_line.clone());

                nearest_line.0
            },
        };

        let mut pos_end = match l.points.iter().position(|p| p.equals(&end)) {
            Some(s) => {
                pos_end_extra = Some((s, l.points[s]));
                s
            },
            None => {

                let ending_point_on_lines = 
                l.points.iter().enumerate().zip(l.points.iter().skip(1))
                .map(|((pos, s0), e0)| {
                    let p_is_on_line = crate::ui::dist_to_segment(end, *s0, *e0);
                    (pos, s0.clone(), e0.clone(), p_is_on_line)
                }).collect::<Vec<_>>();

                let nearest_line = ending_point_on_lines
                    .into_iter()
                    .min_by_key(|f| (f.3.distance * 100000.0).round().abs() as usize)
                    .map(|s| s.clone());
                
                let nearest_line = match nearest_line {
                    Some(s) => s,
                    None => return Vec::new(),
                };
                
                if nearest_line.3.distance > dst {
                    return Vec::new();
                }

                end_is_on_line = Some(nearest_line.clone());

                nearest_line.0
            },
        };
        
        if start_is_on_line.is_some() && end_is_on_line.is_some() {
            return Vec::new(); // TODO - technically wrong, but produces OK results
        }

        let mut startend_swapped = false;
        if pos_end < pos_start {
            std::mem::swap(&mut pos_start, &mut pos_end);
            startend_swapped = true;
        }

        if pos_end.abs_diff(pos_start) < 2 {
            return Vec::new(); // common for regular copied lines
        }

        let normal_direction = pos_end.saturating_sub(pos_start);
        let reverse_direction = pos_start.saturating_add(l.points.len().saturating_sub(pos_end));
        
        if reverse_direction.min(normal_direction) < 2 {
            return Vec::new(); // shared line between two points
        }

        let normal = l.points.iter()
        .skip(pos_start.saturating_add(1))
        .take(normal_direction.saturating_sub(1)).cloned()
        .collect::<Vec<_>>();

        let mut rev = l.points.iter().skip(pos_end.saturating_add(1)).cloned().collect::<Vec<_>>();
        rev.extend(l.points.iter().cloned().take(pos_start));
        rev.reverse();

        let normal_error = normal.iter()
        .map(|s| dist_to_segment(*s, start, end).distance.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

        let reverse_error = rev.iter()
        .map(|s| dist_to_segment(*s, start, end).distance.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);
    
        let reverse = reverse_error < normal_error;

        let mut ret = if reverse {
            rev
        } else {
            normal
        };

        ret.dedup_by(|a, b| a.equals(b));
        
        if pos_end == pos_start {
            return Vec::new();
        }

        if ret.len() > 1 {
            match (l.points.get(pos_start), l.points.get(pos_end)) {
                (
                    Some(l_start), 
                    Some(l_end), 
                ) => {
                    
                    if l_start.equals(l_end) {
                        return Vec::new();
                    }
    
                    let mut line_normal = vec![*l_start];
                    for p in ret.iter() {
                        line_normal.push(*p);
                    }
                    line_normal.push(*l_end);
                    let line_normal_length = line_normal.windows(2)
                    .map(|pts| match &pts {
                        &[a, b] => a.dist(b),
                        _ => 0.0,
                    }).sum::<f64>();

                    let mut line_reverse = vec![*l_start];
                    for p in ret.iter().rev() {
                        line_reverse.push(*p);
                    }
                    line_reverse.push(*l_end);
                    let line_reverse_length = line_reverse.windows(2)
                    .map(|pts| match &pts {
                        &[a, b] => a.dist(b),
                        _ => 0.0,
                    }).sum::<f64>();

                    if startend_swapped {
                        if line_normal_length < line_reverse_length {
                            ret.reverse();
                        }
                    } else {
                        if line_reverse_length < line_normal_length {
                            ret.reverse();
                        }
                    }
                },
                _ => { }
            }    
        }

        ret
    }

    fn check_lines_for_points(l: &[SvgLine], start: &SvgPoint, end: &SvgPoint, log: &mut Vec<String>, dst: f64) -> Vec<SvgPoint> {
        for l in l {
            let v = Self::check_line_for_points(l, start, end, log, dst);
            if !v.is_empty() {
                return v;
            }
        }
        Vec::new()
    }

    pub fn get_line_between_points(&self, start: &SvgPoint, end: &SvgPoint, log: &mut Vec<String>, maxdst_line: f64) -> Vec<SvgPoint> {
        let v = Self::check_lines_for_points(&self.poly.outer_rings, start, end, log, maxdst_line);
        if !v.is_empty() {
            return v;
        }
        let v = Self::check_lines_for_points(&self.poly.outer_rings, start, end, log, maxdst_line);
        if !v.is_empty() {
            return v;
        }
        Vec::new()
    }

    pub fn get_groesse(&self) -> f64 {
        translate_to_geo_poly(&self.poly).0.iter().map(|p| p.signed_area()).sum()
    }

    pub fn get_wirtschaftsart(kuerzel: &str) -> Option<String> {
        let map: BTreeMap<String, NutzungsArt> = include!(concat!(env!("OUT_DIR"), "/nutzung.rs"));
        map.get(kuerzel.trim()).map(|s| s.wia.clone())
    }

    pub fn get_auto_kuerzel(&self, ebene: &str) -> Option<String> {

        let vegetationsmerkmal = self.attributes.get("vegetationsmerkmal").map(|s| s.as_str());
        let zustand = self.attributes.get("zustand").map(|s| s.as_str());
        let nutzung = self.attributes.get("nutzung").map(|s| s.as_str());
        let abbaugut = self.attributes.get("abbaugut").map(|s| s.as_str());
        let funktion = self.attributes.get("funktion").map(|s| s.as_str());
        let foerdergut = self.attributes.get("foerdergut").map(|s| s.as_str());
        let primaerenergie = self.attributes.get("primaerenergie").map(|s| s.as_str());
        let art = self.attributes.get("art").map(|s| s.as_str());
        let oberflaechenmaterial = self.attributes.get("oberflaechenmaterial").map(|s| s.as_str());

        match ebene {
            "AX_Wohnbauflaeche" => match funktion {
                Some("1200") => Some("WBFPA"),
                _ => Some("WBF")
            },
            "AX_Landwirtschaft" => match vegetationsmerkmal {
                Some("1010") => Some("A"),
                Some("1011") => Some("SOA"),
                Some("1012") => Some("HOP"),
                Some("1013") => Some("HOP"), // Spargel
                Some("1020") => Some("GR"),
                Some("1021") => Some("SOW"),
                Some("1022") => Some("SAW"),
                Some("1030") => Some("G"),
                Some("1031") => Some("BAUM"),
                Some("1040") => Some("REB"),
                Some("1050") => Some("OBP"),
                Some("1051") => Some("OBBP"),
                Some("1060") => Some("WEIH"),
                Some("1100") => Some("KURZ"),
                Some("1200") => Some("BRA"),
                _ => Some("LW"),
            },
            "AX_Wald" => match (vegetationsmerkmal, nutzung, zustand) {
                (Some("1100"), Some("3000"), Some("6100")) => Some("WABFVNL"),
                (Some("1100"), Some("1000"), Some("6100")) => Some("VNFL"),
                (Some("1200"), Some("3000"), Some("6100")) => Some("WABFVNN"),
                (Some("1200"), Some("1000"), Some("6100")) => Some("VNFN"),
                (Some("1300"), Some("3000"), Some("6100")) => Some("WABFVLN"),
                (Some("1300"), Some("1000"), Some("6100")) => Some("VNFLN"),
                (Some("1100"), _, _) => Some("LH"),
                (Some("1200"), _, _) => Some("NH"),
                (Some("1300"), _, _) => Some("LNH"),
                (_, Some("3000"), Some("6100")) => Some("WABFVLN"),
                (_, Some("1000"), Some("6100")) => Some("VNPF"),
                (_, Some("1000"), _) => Some("FWF"),
                (_, Some("3000"), _) => Some("WABF"),
                _ => Some("WALD"),
            },
            "AX_IndustrieUndGewerbeflaeche" => match (funktion, primaerenergie, foerdergut) {
                (Some("2500"), Some("1000"), _) => Some("VSWA"), // VSWA
                (Some("2500"), Some("2000"), _) => Some("VSKK"), // VSKK
                (Some("2500"), Some("3000"), _) => Some("VSSO"), // VSSO
                (Some("2500"), Some("4000"), _) => Some("VSWI"), // VSWI
                (Some("2500"), Some("7000"), _) => Some("VSVE"), // VSVE
                (Some("2500"), Some("7100"), _) => Some("VSKO"), // VSKO
                (Some("2700"), _, Some("2000")) => Some("FÖG"), // FÖG
                (Some("2530"), Some("1000"), _) => Some("VSKWA"), // VSKWA
                (Some("2530"), Some("2000"), _) => Some("VSKKK"), // VSKKK
                (Some("2530"), Some("3000"), _) => Some("VSKSO"), // VSKSO
                (Some("2530"), Some("4000"), _) => Some("VSKWI"), // VSKWI
                (Some("2530"), Some("7000"), _) => Some("VSKVE"), // VSKVE
                (Some("2530"), Some("7100"), _) => Some("VSKKO"), // VSKKO
                (Some("2570"), Some("1000"), _) => Some("VSHWA"), // VSHWA
                (Some("2570"), Some("2000"), _) => Some("VSHKK"), // VSHKK
                (Some("2570"), Some("3000"), _) => Some("VSHSO"), // VSHSO
                (Some("2570"), Some("4000"), _) => Some("VSHWI"), // VSHWI // Heizwerk - Wind fehlt?
                (Some("2570"), Some("7000"), _) => Some("VSHVE"), // VSHVE
                (Some("2570"), Some("7100"), _) => Some("VSHKO"), // VSHKO
                (Some("2700"), _, _) => Some("FÖ"), // FÖ
                (Some("2520"), _, _) => Some("VSW"), // VSW
                (Some("2530"), _, _) => Some("VSK"), // VSK
                (Some("2540"), _, _) => Some("VSU"), // VSU
                (Some("2570"), _, _) => Some("VSH"), // VSH
                (Some("1200"), _, _) => Some("IGFPA"), // Parken,
                (Some("1400"), _, _) => Some("HD"), // HD,
                (Some("1440"), _, _) => Some("HDH"), // HDH
                (Some("1450"), _, _) => Some("HDM"), // HDM
                (Some("1490"), _, _) => Some("HDG"), // HDG
                (Some("2500"), _, _) => Some("VS"), // VS
                (Some("2600"), _, _) => Some("ES"), // ES
                (Some("2610"), _, _) => Some("ESA"), // ESA
                (Some("2630"), _, _) => Some("ESDO"), // ESDO
                (Some("2640"), _, _) => Some("ESDU"), // ESDU
                _ => Some("IG")
            },
            "AX_TagebauGrubeSteinbruch" => match (abbaugut, funktion, zustand) {
                (Some("1001"), _, Some("2100")) => Some("TGTAB"),
                (Some("1001"), _, _) => Some("TGT"),
                (Some("1004"), _, Some("2100")) => Some("TGLAB"),
                (Some("1004"), _, _) => Some("TGL"),
                (Some("1008"), _, Some("2100")) => Some("TGSAB"),
                (Some("1008"), _, _) => Some("TGS"),
                (Some("1009"), _, Some("2100")) => Some("TGKIAB"),
                (Some("1009"), _, _) => Some("TGKI"),
                (Some("1012"), _, Some("2100")) => Some("TGQAB"),
                (Some("1012"), _, _) => Some("TGQ"),
                (Some("2005"), _, Some("2100")) => Some("TGKSAB"),
                (Some("2005"), _, _) => Some("TGKS"),
                (Some("2010"), _, Some("2100")) => Some("TGGAB"),
                (Some("2010"), _, _) => Some("TGG"),
                (Some("4010"), _, Some("2100")) => Some("TGTFAB"),
                (Some("4010"), _, _) => Some("TGTF"),
                (Some("4021"), _, Some("2100")) => Some("TGBAB"),
                (Some("4021"), _, _) => Some("TGB"),
                (_, Some("1200"), _) => Some("TGPA"),
                _ => Some("TG"),
            },
            "AX_Bergbaubetrieb" => match zustand {
                Some("2100") => Some("BEAB"),
                _ => Some("BE"),
            },
            // Attribute unklar - Kletterpark, Reitsport, ...?
            "AX_SportFreizeitUndErholungsflaeche" => match funktion {
                Some("4100") => Some("SFS"),
                Some("4110") => Some("SFG"),
                Some("4200") => Some("SFZ"),
                Some("4220") => Some("SFWP"),
                Some("4240") => Some("SFB"),
                Some("4250") => Some("SFM"),
                Some("4260") => Some("SFA"),
                Some("4290") => Some("SFMO"),
                Some("4300") => Some("EH"),
                Some("4310") => Some("WFH"),
                Some("4320") => Some("SCHW"),
                Some("4330") => Some("CAM"),
                Some("4400") => Some("GRÜ"),
                Some("4420") => Some("PARK"),
                Some("4440") => Some("SFPA"),
                _ => Some("SF"),
            },
            "AX_FlaecheGemischterNutzung" => match funktion {
                Some("1200") => Some("MIPA"),
                Some("6800") => Some("MILB"),
                Some("7600") => Some("MIFB"),
                Some("3000") => Some("MIFW"),
                _ => Some("MI"),
            },
            "AX_FlaecheBesondererFunktionalerPraegung" => match funktion {
                Some("1100") => Some("BPÖ"),
                Some("1110") => Some("BPV"),
                Some("1120") => Some("BPB"),
                Some("1130") => Some("BPK"),
                Some("1140") => Some("BPR"),
                Some("1150") => Some("BPG2"),
                Some("1160") => Some("BPS"),
                Some("1170") => Some("BPO2"),
                Some("1200") => Some("BPPA"),
                Some("1300") => Some("BPHA"),
                _ => Some("BP"),
            },
            "AX_Friedhof" => match funktion {
                Some("1200") => Some("FPA"),
                _ => Some("F"),
            },
            "AX_Strassenverkehr" => match funktion {
                Some("2312") => Some("SB"),
                Some("5130") => Some("FUß"),
                _ => Some("S"),
            },
            "AX_Weg" => Some("WEG"),
            "AX_Platz" => match funktion {
                Some("5310") => Some("P"),
                Some("5320") => Some("RAP"),
                Some("5330") => Some("RAS"),
                Some("5350") => Some("FP"),
                _ => Some("PL")
            },
            "AX_Bahnverkehr" => match funktion {
                Some("1200") => Some("BAPA"),
                Some("2322") => Some("BAB"),
                _ => Some("BA"),
            },
            "AX_Flugverkehr" => match (art, funktion) {
                (Some("5511"), _) => Some("IFH"),
                (Some("5512"), _) => Some("RFH"),
                (Some("5513"), _) => Some("SFH"),
                (Some("5521"), _) => Some("VLP"),
                (Some("5522"), _) => Some("SLP"),
                (Some("5530"), _) => Some("HLP"),
                (Some("5550"), _) => Some("SFLG"),
                (_, Some("1200")) => Some("FLPA"),
                _ => Some("FL"),
            },
            "AX_Schiffsverkehr" => match funktion {
                Some("1200") => Some("SVPA"),
                Some("5610") => Some("SVH"),
                Some("5620") => Some("SVS"),
                _ => Some("SV")
            }
            "AX_Gehoelz" => Some("GHÖ"),
            "AX_Heide" => Some("HEI"),
            "AX_Moor" => Some("MOOR"),
            "AX_Sumpf" => Some("SUM"),
            "AX_Halde" => Some("HAL"),
            "AX_UnlandVegetationsloseFlaeche" => match (funktion, oberflaechenmaterial) {
                (Some("1000"), Some("1020")) => Some("UVS"),
                (Some("1000"), Some("1030")) => Some("UVS"),
                (Some("1000"), Some("1040")) => Some("UVSA"),
                (Some("1100"), _) => Some("WAB"),
                (Some("1300"), _) => Some("NF"),
                _ => Some("UV"),
            },
            "AX_Fliessgewaesser" => match funktion {
                Some("8300") => Some("KAN"),
                _ => Some("WAF"),
            },
            "AX_Hafenbecken" => Some("WAH"),
            "AX_StehendesGewaesser" => match funktion {
                Some("8630") => Some("STS"),
                Some("8631") => Some("SPB"),
                _ => Some("WAS")
            },
            _ => None,
        }.map(|s| s.to_string())
    }

    pub fn get_rect(&self) -> quadtree_f32::Rect {
        self.poly.get_rect()
    }

    pub fn get_fit_bounds(&self) -> [[f64;2];2] {
        self.poly.get_fit_bounds()
    }
}

impl SvgPolygon {

    pub fn get_rect(&self) -> quadtree_f32::Rect {
        let [[min_y, min_x], [max_y, max_x]] = self.get_fit_bounds();
        quadtree_f32::Rect {
            max_x: max_x,
            max_y: max_y,
            min_x: min_x,
            min_y: min_y,
        }
    }

    pub fn get_fit_bounds(&self) -> [[f64;2];2] {
        let mut min_x = self.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.x).unwrap_or(0.0);
        let mut max_x = self.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.x).unwrap_or(0.0);
        let mut min_y = self.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.y).unwrap_or(0.0);
        let mut max_y = self.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.y).unwrap_or(0.0);
        for l in self.outer_rings.iter() {
            for p in l.points.iter() {
                if p.x > max_x {
                    max_x = p.x;
                }
                if p.y > max_y {
                    max_y = p.y;
                }
                if p.x < min_x {
                    min_x = p.x;
                }
                if p.y < min_y {
                    min_y = p.y;
                }
            }
        }
    
        [
            [min_y, min_x],
            [max_y, max_x]
        ]
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SvgPolygon {
    pub outer_rings: Vec<SvgLine>,
    pub inner_rings: Vec<SvgLine>,
}

impl SvgPolygon {

    pub fn equals_any_ring(&self, other: &Self) -> bool {
        if self.outer_rings.len() != 1 {
            return false;
        }
        let first_ring = &self.outer_rings[0];

        for or in other.outer_rings.iter() {
            if or.equals(first_ring) {
                return true;
            }
        }

        false
    }

    pub fn translate_y(&self, newy: f64) -> Self {
        Self {
            outer_rings: self.outer_rings.iter().map(|s| SvgLine {
                points: s.points.iter().map(|p| SvgPoint { x: p.x, y: p.y + newy }).collect()
            }).collect(),
            inner_rings: self.inner_rings.iter().map(|s| SvgLine {
                points: s.points.iter().map(|p| SvgPoint { x: p.x, y: p.y + newy }).collect()
            }).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.outer_rings.is_empty() &&
        self.inner_rings.is_empty()
    }
    
    pub fn equals(&self, other: &Self) -> bool {
        self.outer_rings.len() == other.outer_rings.len() &&
        self.inner_rings.len() == other.inner_rings.len() &&
        self.outer_rings.iter().zip(other.outer_rings.iter()).all(|(a, b)| a.equals(b)) &&
        self.inner_rings.iter().zip(other.inner_rings.iter()).all(|(a, b)| a.equals(b))
    }

    fn round_line(s: &SvgLine) -> SvgLine {
        SvgLine { points: s.points.iter().map(SvgPoint::round_to_3dec).collect() }
    }

    pub fn round_to_3dec(&self) -> Self {
        Self {
            outer_rings: self.outer_rings.iter().map(Self::round_line).collect(),
            inner_rings: self.inner_rings.iter().map(Self::round_line).collect(),
        }
    }

    pub fn get_label_pos(&self, tolerance: f64) -> SvgPoint {
        
        let coords_outer = self.outer_rings.iter().flat_map(|line| {
            line.points.iter().map(|p| (p.x, p.y))
       }).collect::<Vec<_>>();

       let polygon = polylabel_mini::Polygon {
           exterior: polylabel_mini::LineString {
               points: coords_outer.iter().map(|(x, y)| polylabel_mini::Point {
                   x: *x,
                   y: *y,
               }).collect()
           },
           interiors: self.inner_rings.iter().map(|l| polylabel_mini::LineString {
               points: l.points.iter().map(|p| polylabel_mini::Point {
                   x: p.x,
                   y: p.y,
               }).collect()
           }).collect()
       };

       let label_pos = polylabel_mini::polylabel(&polygon, tolerance);
       
       SvgPoint {
            x: label_pos.x,
            y: label_pos.y,
       }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SvgLine {
    pub points: Vec<SvgPoint>,
}

impl SvgLine {

    /// Return the two points describing a side of this polygon. Indexing from zero.
    pub fn get_side(&self, i: usize) -> (SvgPoint, SvgPoint) {
        let p1 = self.points.get(i).cloned().unwrap_or_default();
        // handle that the polygon wraps around back to the start.
        let p2: SvgPoint = if i + 1 >= self.points.len() {
            self.points.get(0).cloned().unwrap_or_default()
        } else {
            self.points.get(i + 1).cloned().unwrap_or_default()
        };

        (p1, p2)
    }
    
    pub fn equals(&self, other: &Self) -> bool {
        self.points.len() == other.points.len() &&
        self.points.iter().zip(other.points.iter()).all(|(a, b)| a.equals(b))
    }

    pub fn get_rect(&self) -> quadtree_f32::Rect {
        SvgPolygon {
            outer_rings: vec![self.clone()],
            inner_rings: Vec::new(),
        }.get_rect()
    }
}

#[derive(Debug, Copy, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SvgPoint {
    pub x: f64,
    pub y: f64,
}

impl SvgPoint {

    /// Return the angle in radians to another point
    pub fn angle_to(&self, other: &SvgPoint) -> f64 {
        let translated = other.translate(&self.invert());

        let result = translated.y.atan2(translated.x);
        if result < 0.0 {
            return result + 360.0_f64.to_radians();
        }
        result
    }

    /// offset / translate this point by another one.
    pub fn translate(&self, by: &Point) -> Point {
        Point {
            x: self.x + by.x,
            y: self.y + by.y,
        }
    }

    /// Flip the sign of both x and y coords
    pub fn invert(&self) -> Point {
        Point {
            x: -self.x,
            y: -self.y,
        }
    }

    pub fn dist(&self, other: &Self) -> f64 {
        crate::ui::dist(*self, *other)
    }

    #[inline]
    pub fn round_f64(f: f64) -> f64 {
        (f * 1000.0).round() / 1000.0
    }

    #[inline]
    pub fn round_to_3dec(&self) -> Self {
        Self {
            x: Self::round_f64(self.x),
            y: Self::round_f64(self.y),
        }
    }

    pub fn equals(&self, other: &Self) -> bool {
        approx_eq!(f64, self.x, other.x, epsilon = 0.001) &&
        approx_eq!(f64, self.y, other.y, epsilon = 0.001)
    }

    pub fn get_rect(&self, dst: f64) -> quadtree_f32::Rect {
        quadtree_f32::Rect {
            max_x: self.x + dst,
            min_x: self.x - dst,
            max_y: self.y + dst,
            min_y: self.y - dst,
        }
    }
}

impl Eq for SvgPoint { }

impl Ord for SvgPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let ax = (self.x * 1000.0) as usize;
        let bx = (other.x * 1000.0) as usize;
        let ay = (self.y * 1000.0) as usize;
        let by = (other.y * 1000.0) as usize;
        if ax == bx && ay == by {
            std::cmp::Ordering::Equal
        } else if ax < bx || ay < by {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    }
}

/// Parse the XML, returns [AX_Gebauede => (Polygon)]
pub fn parse_nas_xml(s: Vec<XmlNode>, whitelist: &[String], log: &mut Vec<String>) -> Result<NasXMLFile, String> {
    xml_nodes_to_nas_svg_file(s, whitelist)
}

fn xml_nodes_to_nas_svg_file(xml: Vec<XmlNode>, whitelist: &[String]) -> Result<NasXMLFile, String> {

    // CRS parsen

    let mut crs: Option<String> = None;
    let crs_nodes = get_all_nodes_in_subtree(&xml, "AA_Koordinatenreferenzsystemangaben");
    for c in crs_nodes {
        match get_all_nodes_in_subtree(&c.children, "standard").first() {
            Some(XmlNode { text: Some(s), .. }) if s == "true" => { },
            _ => continue,
        }
        let nodes = get_all_nodes_in_subtree(&c.children, "crs");
        let cs = match nodes.first() {
            Some(s) => s,
            None => continue,
        };
        crs = cs.attributes.get("href").cloned();
    }
    let crs = match crs {
        Some(s) => s,
        None => return Err(format!("kein Koordinatenreferenzsystem gefunden (AA_Koordinatenreferenzsystemangaben = standard)")), // no CRS found
    };
    let crs = crs.replace("urn:adv:crs:", "");

    let crs = match get_proj_string(&crs) {
        Some(s) => s,
        None => return Err(format!("Unbekanntes CRS: {crs}")),
    };

    // Objekte parsen
    let whitelist = std::collections::BTreeSet::from_iter(whitelist.iter().cloned());
    let objekte_nodes = get_all_nodes_in_subtree(&xml, "member");
    let mut objekte = BTreeMap::new();
    for o in objekte_nodes.iter() {
        let o_node = match o.children.first() {
            Some(s) => s,
            None => continue,
        };
        if !whitelist.contains(o_node.node_type.as_str()) {
            continue;
        }
        let key = o_node.node_type.clone();
        let patches = get_all_nodes_in_subtree(&o_node.children, "PolygonPatch");
        if patches.is_empty() {
            continue;
        }
        let mut outer_rings = Vec::new();
        let mut inner_rings = Vec::new();
        let children = patches.iter().flat_map(|s| s.children.clone()).collect::<Vec<_>>();
        for e_i in children.iter() {
            let external = match e_i.node_type.as_str() {
                "exterior" => true,
                "interior" => false,
                _ => continue,
            };
            let linestrings = get_all_nodes_in_subtree(&e_i.children, "LineStringSegment");
            let linestring_points = linestrings
                .iter()
                .flat_map(|s| {
                    s.children.iter()
                    .filter_map(|s| s.text.clone())
                    .map(|text| {
                        let pts = text
                        .split_whitespace()
                        .filter_map(|s| s.parse::<f64>().ok())
                        .collect::<Vec<_>>();
                        pts.chunks(2).filter_map(|f| {
                            match f {
                                [east, false_north] => Some(SvgPoint {
                                    x: *east,
                                    y: *false_north,
                                }),
                                _ => None,
                            }
                        }).collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            let mut line_points = linestring_points.into_iter().flat_map(|f| f.into_iter()).collect::<Vec<_>>();
            line_points.dedup();
            if line_points.len() < 3 {
                continue;
            }
            let line = SvgLine {
                points: line_points,
            };
            if external {
                outer_rings.push(line);
            } else {
                inner_rings.push(line);
            }
        }

        if outer_rings.is_empty() && inner_rings.is_empty() {
            continue;
        }
        
        let mut attributes = o_node.children.iter()
        .filter_map(|cn| match &cn.text {
            Some(s) => Some((cn.node_type.clone(), s.clone())),
            None => None,
        }).collect::<BTreeMap<_, _>>();
        attributes.extend(o_node.attributes.clone().into_iter());

        let flst_id = match attributes.get("id") {
            Some(s) => s.clone(),
            None => continue,
        };

        let tp = TaggedPolygon {
            poly: SvgPolygon {
                outer_rings: outer_rings,
                inner_rings: inner_rings,
            },
            attributes,
        };

        objekte.entry(key).or_insert_with(|| Vec::new()).push(tp);
    }

    Ok(NasXMLFile {
        crs: crs,
        ebenen: objekte,
    })
}

fn get_proj_string(input: &str) -> Option<String> {

    let mut known_strings = BTreeMap::new();
    for i in 0..60 {
        known_strings.insert(format!("ETRS89_UTM{i}"), format!("+proj=utm +ellps=GRS80 +units=m +no_defs +zone={i}"));
    }

    known_strings.get(input).cloned()
}


#[derive(Debug, Copy, Clone)]
pub enum UseRadians {
    ForSourceAndTarget,
    ForSource,
    ForTarget,
    None,
}

impl UseRadians {
    fn for_source(&self) -> bool {
        match self {
            UseRadians::ForSourceAndTarget => true,
            UseRadians::ForSource => true,
            UseRadians::ForTarget => false,
            UseRadians::None => false,
        }
    }
    fn for_target(&self) -> bool {
        match self {
            UseRadians::ForSourceAndTarget => true,
            UseRadians::ForSource => false,
            UseRadians::ForTarget => true,
            UseRadians::None => false,
        }
    }
}
pub fn reproject_line(line: &SvgLine, source: &Proj, target: &Proj, use_radians: UseRadians) -> SvgLine {
    SvgLine {
        points: line.points.iter().filter_map(|p| {
            let mut point3d = if use_radians.for_source()  {
                (p.x.to_radians(), p.y.to_radians(), 0.0_f64) 
            } else {
                (p.x, p.y, 0.0_f64) 
            };
            proj4rs::transform::transform(source, target, &mut point3d).ok()?;
            Some(SvgPoint {
                x: if use_radians.for_target() { point3d.0 } else { point3d.0.to_degrees() }, 
                y: if use_radians.for_target() { point3d.1 } else { point3d.1.to_degrees() },
            })
        }).collect()
    }
}

pub fn reproject_poly(
    poly: &SvgPolygon,
    source_proj: &proj4rs::Proj,
    target_proj: &proj4rs::Proj,
    use_radians: UseRadians,
) -> SvgPolygon {
    SvgPolygon {
        outer_rings: poly.outer_rings.iter()
        .map(|l| reproject_line(l, &source_proj, &target_proj, use_radians))
        .collect(),
        inner_rings: poly.inner_rings.iter()
        .map(|l| reproject_line(l, &source_proj, &target_proj, use_radians))
        .collect(),
    }
}

pub fn transform_nas_xml_to_lat_lon(input: &NasXMLFile, log: &mut Vec<String>) -> Result<NasXMLFile, String> {
    let source_proj = Proj::from_proj_string(&input.crs)
    .map_err(|e| format!("source_proj_string: {e}: {:?}", input.crs))?;
    
    let latlon_proj = Proj::from_proj_string(LATLON_STRING)
    .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    let objekte = input.ebenen.iter()
    .map(|(k, v)| {
        (k.clone(), v.iter().map(|v| {
            TaggedPolygon {
                attributes: v.attributes.clone(),
                poly: reproject_poly(&v.poly, &source_proj, &latlon_proj, UseRadians::None)
            }
        }).collect())
    }).collect();

    Ok(NasXMLFile {
        ebenen: objekte,
        crs: LATLON_STRING.to_string(),
    })
}

pub fn transform_split_nas_xml_to_lat_lon(input: &SplitNasXml, log: &mut Vec<String>) -> Result<SplitNasXml, String> {
    let source_proj = Proj::from_proj_string(&input.crs).map_err(|e| format!("source_proj_string: {e}: {:?}", input.crs))?;
    let latlon_proj_string = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";
    let latlon_proj = Proj::from_proj_string(latlon_proj_string).map_err(|e| format!("latlon_proj_string: {e}: {latlon_proj_string:?}"))?;

    let flurstuecke_nutzungen = input.flurstuecke_nutzungen.iter()
    .map(|(k, v)| {
        (k.clone(), v.iter().map(|v| {
            TaggedPolygon {
                attributes: v.attributes.clone(),
                poly: SvgPolygon {
                    outer_rings: v.poly.outer_rings.iter()
                    .map(|l| reproject_line(l, &source_proj, &latlon_proj, UseRadians::None))
                    .collect(),
                    inner_rings: v.poly.inner_rings.iter()
                    .map(|l| reproject_line(l, &source_proj, &latlon_proj, UseRadians::None))
                    .collect(),
                }
            }
        }).collect())
    }).collect();

    Ok(SplitNasXml {
        flurstuecke_nutzungen: flurstuecke_nutzungen,
        crs: latlon_proj_string.to_string(),
    })
}

pub fn fixup_flst_groesse(unprojected: &SplitNasXml, projected: &mut SplitNasXml) {
    for (key, up_polys) in unprojected.flurstuecke_nutzungen.iter() {
        let mut p_polys = match projected.flurstuecke_nutzungen.get_mut(key) {
            Some(s) => s,
            None => continue,
        };
        for (up, p) in up_polys.iter().zip(p_polys.iter_mut()) {
            let up_size = up.get_groesse();
            p.attributes.insert("BerechneteGroesseM2".to_string(), up_size.round().to_string());
        }
    }
}

pub type FlstId = String;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitNasXml {
    pub crs: String,
    // Flurstücke, indexiert nach Flst ID, mit Nutzungen als Polygonen
    pub flurstuecke_nutzungen: BTreeMap<FlstId, Vec<TaggedPolygon>>,
}

impl SplitNasXml {
    pub fn get_flst_part_by_id(&self, flstpartid: &str) -> Option<&TaggedPolygon> {
        let split = flstpartid.split(":").collect::<Vec<_>>();
        let (ax_flurstueck, ax_ebene, cut_obj_id) = match &split[..] {
            &[a, e, o] => (a, e, o),
            _ => return None,
        };
        self.flurstuecke_nutzungen.get(ax_flurstueck)?
        .iter()
        .find(|p| {
            p.attributes.get("AX_Ebene").map(|s| s.as_str()) == Some(ax_ebene) &&
            p.attributes.get("id").map(|s| s.as_str()) == Some(cut_obj_id)
        })
    }
}

#[derive(Debug, Clone)]
pub struct NasXmlQuadTree {
    pub items: usize,
    original: NasXMLFile,
    qt: quadtree_f32::QuadTree,
    ebenen_map: BTreeMap<ItemId, (FlstId, usize)>,
}

impl NasXmlQuadTree {
    pub fn get_overlapping_flst(&self, rect: &quadtree_f32::Rect) -> Vec<TaggedPolygon> {
        self.qt.get_ids_that_overlap(rect)
        .into_iter()
        .filter_map(|itemid| {
            let (flst_id, i) = self.ebenen_map.get(&itemid)?;
            self.original.ebenen.get(flst_id)?.get(*i).cloned()
        }).collect()
    }

    // return = empty if points not on any flst line
    pub fn get_line_between_points(&self, start: &SvgPoint, end: &SvgPoint, log: &mut Vec<String>, maxdst_line: f64, maxdst_line2: f64) -> Vec<SvgPoint> {
        let mut polys = self.get_overlapping_flst(&start.get_rect(maxdst_line));
        polys.extend(self.get_overlapping_flst(&end.get_rect(maxdst_line)));
        polys.sort_by(|a, b| a.attributes.get("id").cmp(&b.attributes.get("id")));
        polys.dedup_by(|a, b| a.attributes.get("id") == b.attributes.get("id"));
        for p in polys {
            let v = p.get_line_between_points(start, end, log, maxdst_line2);
            if !v.is_empty() {
                return v;
            }
        }
        Vec::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitNasXmlQuadTree {
    pub items: usize,
    original: SplitNasXml,
    qt: quadtree_f32::QuadTree,
    flst_nutzungen_map: BTreeMap<ItemId, (FlstId, usize)>,
}

impl SplitNasXml {
    pub fn get_polyline_guides_in_bounds(&self, search_bounds: quadtree_f32::Rect) -> Vec<SvgLine> {
        let mut b = Vec::new();
        for (_k, v) in self.flurstuecke_nutzungen.iter() {
            for poly in v {
                let [[min_y, min_x], [max_y, max_x]] = poly.get_fit_bounds();
                let bounds = Rect {
                    max_x: max_x,
                    max_y: max_y,
                    min_x: min_x,
                    min_y: min_y,
                };
                if bounds.overlaps_rect(&search_bounds) {
                    b.extend(poly.poly.outer_rings.iter().cloned());
                    b.extend(poly.poly.inner_rings.iter().cloned());
                }
            }
        }
        b
    }

    pub fn create_quadtree(&self) -> SplitNasXmlQuadTree {

        let mut flst_nutzungen_map = BTreeMap::new();
        let mut items = BTreeMap::new();
        let mut itemid = 0;
        for (flst_id, polys) in self.flurstuecke_nutzungen.iter() {
            for (i, p) in polys.iter().enumerate() {
                let id = ItemId(itemid);
                itemid += 1;
                items.insert(id, Item::Rect(p.get_rect()));
                flst_nutzungen_map.insert(id, (flst_id.clone(), i));
            }
        }
        
        let qt = QuadTree::new(items.into_iter());

        SplitNasXmlQuadTree {
            items: itemid + 1,
            original: self.clone(),
            qt: qt,
            flst_nutzungen_map,
        }
    }
}

impl SplitNasXmlQuadTree {
    pub fn get_overlapping_flst(&self, rect: &quadtree_f32::Rect) -> Vec<TaggedPolygon> {
        self.qt.get_ids_that_overlap(rect)
        .into_iter()
        .filter_map(|itemid| {
            let (flst_id, i) = self.flst_nutzungen_map.get(&itemid)?;
            self.original.flurstuecke_nutzungen.get(flst_id)?.get(*i).cloned()
        }).collect()
    }
}

pub fn split_xml_flurstuecke_inner(input: &NasXMLFile, log: &mut Vec<String>) -> Result<SplitNasXml, String> {

    let mut input = input.clone();
    let mut default = SplitNasXml {
        crs: input.crs.clone(),
        flurstuecke_nutzungen: BTreeMap::new(),
    };
    let ax_flurstuecke = input.ebenen.remove("AX_Flurstueck").unwrap_or_default();
    let _ = input.ebenen.remove("AX_Gebaeude");
    let _ = input.ebenen.remove("AX_HistorischesFlurstueck");
    if ax_flurstuecke.is_empty() {
        default.crs = "empty ax flurstuecke!".to_string();
        return Ok(default);
    }

    let mut btree_id_to_poly = BTreeMap::new();
    let mut itemid = 0_usize;
    for (k, polys) in input.ebenen.iter() {
        for poly in polys.iter() {
            let mut poly = poly.clone();
            poly.attributes.insert("AX_Ebene".to_string(), k.clone());
            btree_id_to_poly.insert(itemid, poly);
            itemid += 1;
        }
    }

    let nutzungs_qt = QuadTree::new(btree_id_to_poly.iter().map(|(k, v)| {
        let [[min_y, min_x], [max_y, max_x]] = v.get_fit_bounds();
        let bounds = Rect {
            max_x: max_x,
            max_y: max_y,
            min_x: min_x,
            min_y: min_y,
        };
        (ItemId(*k), Item::Rect(bounds))
    }));

    let flurstuecke_nutzungen = ax_flurstuecke.iter().filter_map(|flst| {

        let id = flst.attributes.get("flurstueckskennzeichen")?.replace("_", "");
        let id = FlstIdParsed::from_str(&id).parse_num()?.format_start_str();

        let [[min_y, min_x], [max_y, max_x]] = flst.get_fit_bounds();
        let bounds = Rect {
            max_x: max_x,
            max_y: max_y,
            min_x: min_x,
            min_y: min_y,
        };
        let ids = nutzungs_qt.get_ids_that_overlap(&bounds);
        let polys = ids.iter().filter_map(|i| btree_id_to_poly.get(&i.0)).collect::<Vec<_>>();

        let polys = polys.iter().flat_map(|p| {
            let intersection_mp = intersect_polys(&flst.poly, &p.poly);
            intersection_mp.into_iter().map(|svg_poly| TaggedPolygon {
                attributes: {
                    let mut attrs = p.attributes.clone();
                    attrs.insert("AX_Flurstueck".to_string(), id.clone());
                    attrs
                },
                poly: svg_poly,
            })
        }).collect::<Vec<_>>();

        if polys.is_empty() {
            None
        } else {
            Some((id.clone(), polys))
        }
    }).collect();

    Ok(SplitNasXml {
        crs: input.crs.clone(),
        flurstuecke_nutzungen,
    })
}

pub fn intersect_polys(a: &SvgPolygon, b: &SvgPolygon) -> Vec<SvgPolygon> {
    use geo::BooleanOps;
    let a = a.round_to_3dec();
    let b = b.round_to_3dec();
    // TODO: nas::only_touches crashes here???
    if a.equals_any_ring(&b) {
        return vec![a];
    }
    if b.equals_any_ring(&a) {
        return vec![b];
    }
    let a = translate_to_geo_poly(&a);
    let b = translate_to_geo_poly(&b);
    let intersect = a.intersection(&b);
    translate_from_geo_poly(&intersect)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgPolyInternalResult {
    pub points_touching_lines: usize,
    pub points_inside_other_poly: usize,
    pub all_points_are_on_line: bool,
}


fn point_in_line(p: &SvgPoint, l: &SvgLine) -> bool {

    // work out the sum of the angles between adjacent points and the point we are checking.
    // if the sum is equal to 360 degrees then we are inside the polygon.
    let mut total = 0.0;

    for i in 0..l.points.len() {
        let (p1, p2) = l.get_side(i);
        let angle_a = p.angle_to(&p2);
        let angle_b = p.angle_to(&p1);

        // handle rolling around over the 360/0 degree line reasonably
        let result = if angle_a > angle_b {
            -((360.0_f64.to_radians() - angle_a) + angle_b)
        } else {
            angle_a - angle_b
        };

        total += result;
    }
    approx_eq!(f64, total.abs(), 360.0_f64.to_radians(), ulps = 2)
}

fn point_is_in_polygon(p: &SvgPoint, poly: &SvgPolygon) -> bool {
    
    if !poly.get_rect().contains_point(&quadtree_f32::Point {
        x: p.x,
        y: p.y,
    }) {
        return false;
    }

    let mut c_in_outer = false;
    for o in poly.outer_rings.iter() {
        if point_in_line(p, o) {
            c_in_outer = true;
            break;
        }
    }

    if (c_in_outer) {
        for i in poly.inner_rings.iter() {
            if point_in_line(p, i) {
                c_in_outer = false;
                break;
            }
        }
    }

    c_in_outer
}

pub fn only_touches(a: &SvgPolygon, b: &SvgPolygon) -> bool {
    let is_1 = only_touches_internal(a, b);
    let is_2 = only_touches_internal(b, a);
    // no intersection of the two polygons possible
    if is_1.points_inside_other_poly == 0 && is_2.points_inside_other_poly == 0 {
        if is_1.all_points_are_on_line || is_2.all_points_are_on_line {
            // a is a subset of b or b is a subset of a
            false
        } else {
            true
        }
    } else {
        false
    }
}

// Only touches the other polygon but does not intersect
pub fn only_touches_internal(a: &SvgPolygon, b: &SvgPolygon) -> SvgPolyInternalResult {

    let points_a = a.outer_rings.iter().flat_map(|l| l.points.iter()).collect::<Vec<_>>();
    let b_geo = translate_to_geo_poly(b);

    let mut points_touching_lines = 0;
    let mut points_inside_other_poly = 0;
    let mut all_points_are_on_line = true;
    for start_a in points_a.iter() {
        if point_is_on_any_line(start_a, &b) {
            points_touching_lines += 1;
        } else if point_is_in_polygon(*start_a, &b) {
            points_inside_other_poly += 1;
            all_points_are_on_line = false;
        } else {
            all_points_are_on_line = false;
        }
    }

    SvgPolyInternalResult {
        points_touching_lines,
        points_inside_other_poly,
        all_points_are_on_line
    }
}

fn point_is_on_any_line(p: &SvgPoint, poly: &SvgPolygon) -> bool {
    for line in poly.outer_rings.iter() {
        for q in line.points.windows(2) {
            match &q {
                &[sa, eb] => {
                    if dist_to_segment(*p, *sa, *eb).distance < 0.01 {
                        return true;
                    }
                },
                _ => { }
            }
        }
    }
    false
}

pub fn translate_to_geo_poly(a: &SvgPolygon) -> geo::MultiPolygon<f64> {
    geo::MultiPolygon(a.outer_rings.iter().map(|outer| {
        let outer = translate_geoline(outer);
        let inner = a.inner_rings.iter().map(translate_geoline).collect::<Vec<_>>();
        geo::Polygon::new(outer, inner)
    }).collect())
}

fn translate_geoline(a: &SvgLine) -> geo::LineString<f64> {
    geo::LineString(a.points.iter().map(|coord| geo::Coord {
        x: coord.x,
        y: coord.y,
    }).collect())
}

pub fn translate_from_geo_poly(a: &geo::MultiPolygon<f64>) -> Vec<SvgPolygon> {
    a.0.iter().map(|s| {
        SvgPolygon {
            outer_rings: vec![translate_ring(s.exterior())],
            inner_rings: s.interiors().iter().map(translate_ring).collect(),
        }
    }).collect()
}

fn translate_ring(a: &geo::LineString<f64>) -> SvgLine {
    SvgLine {
        points: a.coords_iter().into_iter().map(|coord| SvgPoint {
            x: coord.x,
            y: coord.y,
        }).collect(),
    }
}