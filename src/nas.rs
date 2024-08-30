use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Split;
use float_cmp::approx_eq;
use float_cmp::ApproxEq;
use float_cmp::F64Margin;
use geo::Area;
use geo::Centroid;
use geo::ConvexHull;
use geo::CoordsIter;
use geo::TriangulateEarcut;
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
use crate::geograf::points_to_rect;
use crate::search::NutzungsArt;
use crate::ui::dist_to_segment;
use crate::ui::Aenderungen;
use crate::uuid_wasm::log_status;
use crate::uuid_wasm::log_status_clear;
use crate::xlsx::FlstIdParsed;
use crate::xml::XmlNode;
use crate::xml::get_all_nodes_in_subtree;
use proj4rs::Proj;
use crate::geograf::LinienQuadTree;

pub const LATLON_STRING: &str = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NasXMLFile {
    pub ebenen: BTreeMap<String, Vec<TaggedPolygon>>,
    #[serde(default = "default_etrs33")]
    pub crs: String,
}

impl Default for NasXMLFile {
    fn default() -> Self {
        Self {
            ebenen: BTreeMap::new(),
            crs: default_etrs33(),
        }
    }
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
    pub flst_id: Vec<String>,
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

        let ax_flurstuecke = match self.ebenen.get("AX_Flurstueck") {
            Some(o) => o,
            None => return format!("keine Ebene AX_Flurstueck vorhanden"),
        };

        let ax_gebaeude = match self.ebenen.get("AX_Gebaeude") {
            Some(o) => o,
            None => return format!("keine Ebene AX_Gebaeude vorhanden"),
        };

        // Flurstueck_ID => Flurstueck Poly
        let ax_flurstuecke_map = ax_flurstuecke.iter().filter_map(|tp| {
            let flst_id = tp.attributes.get("flurstueckskennzeichen").cloned()?;
            let rect = tp.get_rect();
            Some((flst_id, rect, &tp.poly))
        }).collect::<Vec<(_, _, _)>>();

        let gebaeude_avail = ax_gebaeude.iter().enumerate().filter_map(|(i, tp)| {
            
            let gebaeude_id = tp.attributes.get("id").cloned()?;
            let flst_rect = tp.get_rect();
            let flst = ax_flurstuecke_map.iter()
            .filter(|(id, r, poly)| flst_rect.overlaps_rect(r))
            .filter(|(id, r, poly)| {
                crate::nas::relate(poly, &tp.poly, 1.0).overlaps()
            })
            .map(|(id, _, _)| id.clone())
            .collect::<Vec<_>>();

            Some((gebaeude_id.clone(), GebaeudeInfo {
                flst_id: flst.clone(),
                deleted: aenderungen.gebaeude_loeschen.values().any(|v| v.gebaeude_id == gebaeude_id),
                gebaeude_id: gebaeude_id.clone(),
                poly: tp.clone(),
            }))
        }).collect::<BTreeMap<_, _>>();

        let geom = gebaeude_avail.iter().filter_map(|(k, v)| {

            let holes = v.poly.poly.inner_rings.iter()
            .map(convert_svgline_to_string)
            .collect::<Vec<_>>()
            .join(",");

            let mut attrs = v.poly.attributes.clone();

            attrs.insert("gebaeude_flst_id".to_string(), serde_json::to_string(&v.flst_id).unwrap_or_default());
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

            let label_pos = match o.poly.get_label_pos() {
                Some(s) => s,
                None => continue,
            };

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
        maxdev_followline: f64,
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

        let len_original = start.dist(&end);
        let mut len_merged_points = vec![start];
        len_merged_points.extend(ret.iter().cloned());
        len_merged_points.push(end);
        let len_merged = len_merged_points.windows(2).map(|w| match &w {
            &[a, b] => a.dist(b),
            _ => 0.0,
        }).sum::<f64>();
        
        ret.retain(|p| !p.equals(&start) && !p.equals(&end));

        if len_original + maxdev_followline > len_merged {
            ret
        } else {
            Vec::new()
        }
    }

    fn check_lines_for_points(l: &[SvgLine], start: &SvgPoint, end: &SvgPoint, log: &mut Vec<String>, dst: f64, maxdev_followline: f64) -> Vec<SvgPoint> {
        for l in l {
            let v = Self::check_line_for_points(l, start, end, log, dst, maxdev_followline);
            if !v.is_empty() {
                return v;
            }
        }
        Vec::new()
    }

    pub fn get_line_between_points(
        &self, 
        start: &SvgPoint,
        end: &SvgPoint, 
        log: &mut Vec<String>, 
        maxdst_line: f64,
        maxdev_followline: f64,
    ) -> Vec<SvgPoint> {
        let v = Self::check_lines_for_points(&self.poly.outer_rings, start, end, log, maxdst_line, maxdev_followline);
        if !v.is_empty() {
            return v;
        }
        let v = Self::check_lines_for_points(&self.poly.outer_rings, start, end, log, maxdst_line, maxdev_followline);
        if !v.is_empty() {
            return v;
        }
        Vec::new()
    }

    pub fn get_groesse(&self) -> f64 {
        translate_to_geo_poly(&self.poly).0.iter().map(|p| p.signed_area()).sum()
    }

    pub fn get_wirtschaftsart(kuerzel: &str) -> Option<String> {
        let map = crate::get_map();
        map.get(kuerzel.trim()).map(|s| s.wia.clone())
    }

    pub fn get_nutzungsartenkennung(kuerzel: &str) -> Option<usize> {
        let map = crate::get_map();
        map.get(kuerzel.trim()).and_then(|s| s.nak.parse::<usize>().ok())
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

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SvgPolygon {
    pub outer_rings: Vec<SvgLine>,
    pub inner_rings: Vec<SvgLine>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum EqualsAnyRingStatus {
    EqualToRing(usize),
    TouchesInside,
    TouchesOutside,
    NotEqualToAnyRing,
    OverlapsAndTouches,
    OverlapsWithoutTouching,
    ContainedInside,
    DistinctOutside,
}

impl SvgPolygon {

    pub fn is_inside_of(&self, other: &Self) -> bool {

        let triangle_points = translate_to_geo_poly(&self).0
        .iter().flat_map(|f| f.earcut_triangles()).map(|i| i.centroid())
        .map(|p| SvgPoint { x: p.x(), y: p.y() })
        .collect::<Vec<_>>();
        
        triangle_points.iter().any(|p| point_is_in_polygon(p, other))
    }

    pub fn from_line(l: &SvgLine) -> Self {
        Self { outer_rings: vec![l.clone()], inner_rings: Vec::new() }
    }

    pub fn contains_polygon(&self, other: &Self) -> bool {
        for l in other.outer_rings.iter() {
            for p in l.points.iter() {
                if !point_is_in_polygon(p, self) {
                    return false;
                }
            }
        }
        for l in other.inner_rings.iter() {
            for p in l.points.iter() {
                if !point_is_in_polygon(p, self) {
                    return false;
                }
            }
        }

        true
    }
    
    pub fn get_all_pointcoords_sorted(&self) -> Vec<[usize;2]> {
        let mut v = BTreeSet::new();
        for l in self.outer_rings.iter() {
            for p in l.points.iter() {
                v.insert([(p.x * 1000.0).round() as usize, (p.y * 1000.0).round() as usize]);
            }
        }
        for l in self.inner_rings.iter() {
            for p in l.points.iter() {
                v.insert([(p.x * 1000.0).round() as usize, (p.y * 1000.0).round() as usize]);
            }
        }
        v.into_iter().collect()
    }

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

    pub fn insert_points_from(&mut self, other: &Self, maxdst: f64) {
        self.outer_rings = self.outer_rings.iter().map(|o| o.insert_points_from(other, maxdst)).collect();
        self.inner_rings = self.inner_rings.iter().map(|o| o.insert_points_from(other, maxdst)).collect();
        self.correct_almost_touching_points(other, maxdst, false);
    }

    pub fn correct_almost_touching_points(&mut self, other: &Self, maxdst: f64, correct_points_on_lines: bool) {
       
        let mut other_points = Vec::new();
        let mut other_lines = Vec::new();

        for l in other.outer_rings.iter() {
            for p in l.points.iter() {
                other_points.push(*p);
            }
            if correct_points_on_lines {
                for p in l.points.windows(2) {
                    match p {
                        &[a, b] => other_lines.push((a, b)),
                        _ => {},
                    }
                }
            }
        }

        for l in other.inner_rings.iter() {
            for p in l.points.iter() {
                other_points.push(*p);
            }
            if correct_points_on_lines {
                for p in l.points.windows(2) {
                    match p {
                        &[a, b] => other_lines.push((a, b)),
                        _ => {},
                    }
                }
            }
        }

        let max_items_points = other_points.len().saturating_div(20).max(500);
        let max_items_lines = other_lines.len().saturating_div(20).max(500);

        let qt_points = quadtree_f32::QuadTree::new_with_max_items_per_quad(other_points.iter().enumerate().map(|(i, s)| {
            (ItemId(i), Item::Point(quadtree_f32::Point { x: s.x, y: s.y }))
        }), max_items_points);

        for l in self.outer_rings.iter_mut() {
            for p in l.points.iter_mut() {
                let mut closest_other_point = qt_points.get_points_contained_by(&p.get_rect(maxdst))
                .into_iter()
                .map(|p| SvgPoint { x: p.x, y: p.y })
                .filter(|s| s.dist(&p) < maxdst)
                .collect::<Vec<_>>();
                closest_other_point.sort_by(|a, b| a.dist(&p).total_cmp(&b.dist(&p)));
                if let Some(first) = closest_other_point.first() {
                    *p = *first;
                } else {
                    
                }
            }
        }

        for l in self.inner_rings.iter_mut() {
            for p in l.points.iter_mut() {
                let mut closest_other_point = qt_points.get_points_contained_by(&p.get_rect(maxdst))
                .into_iter()
                .map(|p| SvgPoint { x: p.x, y: p.y })
                .filter(|s| s.dist(&p) < maxdst)
                .collect::<Vec<_>>();
                closest_other_point.sort_by(|a, b| a.dist(&p).total_cmp(&b.dist(&p)));
                if let Some(first) = closest_other_point.first() {
                    *p = *first;
                }
            }
        }


        if !correct_points_on_lines {
            return;
        }

        let qt_lines = quadtree_f32::QuadTree::new_with_max_items_per_quad(
            other_lines.iter().enumerate().map(|(i, s)| {
            (ItemId(i), Item::Rect(points_to_rect(&(s.0, s.1))))
        }), max_items_lines);

        for l in self.outer_rings.iter_mut() {
            for p in l.points.iter_mut() {
                let mut closest_other_lines = qt_lines
                .get_ids_that_overlap(&p.get_rect(maxdst))
                .into_iter()
                .filter_map(|i| other_lines.get(i.0))
                .map(|q| {
                    dist_to_segment(*p, q.0, q.1)
                })
                .filter(|s| s.distance < maxdst)
                .collect::<Vec<_>>();
                closest_other_lines.sort_by(|a, b| a.distance.total_cmp(&b.distance));
                if let Some(first) = closest_other_lines.first() {
                    *p = first.nearest_point;
                }
            }
        }

        for l in self.inner_rings.iter_mut() {
            for p in l.points.iter_mut() {
                let mut closest_other_lines = qt_lines.get_ids_that_overlap(&p.get_rect(maxdst))
                .into_iter()
                .filter_map(|i| other_lines.get(i.0))
                .map(|q| {
                    dist_to_segment(*p, q.0, q.1)
                })
                .filter(|s| s.distance < maxdst)
                .collect::<Vec<_>>();
                closest_other_lines.sort_by(|a, b| a.distance.total_cmp(&b.distance));
                if let Some(first) = closest_other_lines.first() {
                    *p = first.nearest_point;
                }
            }
        }

    }

    pub fn inverse_point_order(&self) -> Self {
        Self {
            outer_rings: self.outer_rings.iter().map(|s| s.inverse_point_order()).collect(),
            inner_rings: self.inner_rings.iter().map(|s| s.inverse_point_order()).collect(),
        }
    }

    pub fn correct_winding_order(&mut self) {
        let normal_winding_area = self.area_m2();
        if normal_winding_area > 1.0 {
            return;
        }
        let reverse_order = self.inverse_point_order();
        if reverse_order.area_m2() > 1.0 {
            *self = reverse_order;
        }
    }

    pub fn is_zero_area(&self) -> bool {
        if self.outer_rings.is_empty() {
            return true;
        }
        let area_m2 = self.area_m2();
        let reverse = self.inverse_point_order().area_m2();
        area_m2 < 1.0 && reverse < 1.0
    }

    pub fn area_m2(&self) -> f64 {
        crate::nas::translate_to_geo_poly(&self).0.iter().map(|p| p.signed_area()).sum::<f64>()
    }

    pub fn equals_any_ring(&self, other: &Self) -> EqualsAnyRingStatus {
        if self.outer_rings.len() != 1 {
            return EqualsAnyRingStatus::NotEqualToAnyRing;
        }
        let first_ring = &self.outer_rings[0];

        for (i, or) in other.outer_rings.iter().enumerate() {
            if or.equals(first_ring) {
                return EqualsAnyRingStatus::EqualToRing(i);
            }

            let points_outside = Self::is_center_inside(first_ring, or);
            let points_inside = Self::is_center_inside(first_ring, or);
            let all_points_on_line = Self::equals_ring_dst(first_ring, or);
            
            if all_points_on_line {
                match (points_outside, points_inside) {
                    (true, false) => return EqualsAnyRingStatus::TouchesOutside,
                    (false, true) => return EqualsAnyRingStatus::TouchesInside,
                    (true, true) => return EqualsAnyRingStatus::OverlapsAndTouches,
                    (false, false) => return EqualsAnyRingStatus::NotEqualToAnyRing,
                }
            } else {
                match (points_outside, points_inside) {
                    (true, false) => return EqualsAnyRingStatus::DistinctOutside,
                    (false, true) => return EqualsAnyRingStatus::ContainedInside,
                    (true, true) => return EqualsAnyRingStatus::OverlapsWithoutTouching,
                    (false, false) => return EqualsAnyRingStatus::NotEqualToAnyRing,
                }
            }
        }

        EqualsAnyRingStatus::NotEqualToAnyRing
    }

    pub fn any_point_outside(a: &SvgLine, b: &SvgLine) -> bool {
        let tr = translate_to_geo_poly(&SvgPolygon::from_line(a));
        let a_poly = match tr.0.get(0) {
            Some(s) => s,
            None => return false,
        };
    
        let mut b_poly = SvgPolygon::from_line(b);
        b_poly.correct_winding_order();

        for tri in a_poly.earcut_triangles_iter() {
            let cen = tri.centroid();
            let cen = SvgPoint { x: cen.x(), y: cen.y() };
            if !point_is_in_polygon(&cen, &b_poly) {
                return true;
            }
        }

        false
    }

    pub fn is_center_inside(a: &SvgLine, b: &SvgLine) -> bool { 
        let tr = translate_to_geo_poly(&SvgPolygon::from_line(a));
        let a_poly = match tr.0.get(0) {
            Some(s) => s,
            None => return false,
        };
    
        let mut b_poly = SvgPolygon::from_line(b);
        b_poly.correct_winding_order();

        for tri in a_poly.earcut_triangles_iter() {
            let cen = tri.centroid();
            let cen = SvgPoint { x: cen.x(), y: cen.y() };
            return point_is_in_polygon(&cen, &b_poly);
        }

        return false;
    }

    fn equals_ring_dst(a: &SvgLine, b: &SvgLine) -> bool {
        
        let mut a_points = a.points.clone();
        a_points.dedup_by(|a, b| a.equals(b));
        
        let mut b_points = b.points.clone();
        b_points.dedup_by(|a, b| a.equals(b));
        
        a_points.iter().all(|a| {
            b_points.iter().any(|p| p.dist(a) < 0.005)
        })
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

    pub fn get_secondary_label_pos(&self) -> Option<SvgPoint> {
        
        if self.is_empty() || self.is_zero_area() {
            return None;
        }

        let first_poly = translate_to_geo_poly(self).0;
        let first_poly = first_poly.first()?;

        let mut triangles = first_poly.earcut_triangles();
        
        triangles.sort_by(|a, b| {
            a.unsigned_area().total_cmp(&b.unsigned_area())
        });

        triangles.pop();
        let center = triangles.pop().map(|second_largest| second_largest.centroid())?;
        Some(SvgPoint { x: center.x(), y: center.y() })
    }

    pub fn get_label_pos(&self) -> Option<SvgPoint> {
        
        if self.is_empty() || self.is_zero_area() {
            return None;
        }

        let rect = self.get_rect();
        let center = rect.get_center();
        let center = SvgPoint {
            x: center.x,
            y: center.y,
        };

        if point_is_in_polygon(&center, &self) {
            return Some(center);
        }

        let first_poly = translate_to_geo_poly(self).0;
        let first_poly = first_poly.first()?;

        let triangles = first_poly.earcut_triangles();
        
        let largest_triangle = triangles.iter()
        .max_by_key(|t| (t.unsigned_area() * 1000.0) as usize)?;

        let center = largest_triangle.centroid();

        Some(SvgPoint { x: center.x(), y: center.y() })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SvgLine {
    pub points: Vec<SvgPoint>,
}

impl SvgLine {

    pub fn get_hash(&self) -> [u64;4] {
        use highway::{HighwayHasher, HighwayHash};
        let rounded = SvgPolygon::from_line(self).round_to_3dec().get_all_pointcoords_sorted();
        let bytes = rounded.iter().flat_map(|[a,b]| {
            let mut a = a.to_le_bytes().to_vec();
            a.extend(b.to_le_bytes().into_iter());
            a
        }).collect::<Vec<_>>();
        let res3: [u64; 4] = HighwayHasher::default().hash256(&bytes);
        res3
    }

    pub fn inverse_point_order(&self) -> SvgLine {
        SvgLine {
            points: {
                let mut newp = self.points.clone();
                newp.reverse();
                newp
            }
        }
    }

    pub fn insert_points_from(&self, other: &SvgPolygon, maxdst: f64) -> SvgLine {
        use crate::geograf::l_to_points;
        let mut other_lines = other.outer_rings.iter().flat_map(|ol| l_to_points(ol)).collect::<Vec<_>>();
        other_lines.extend(other.inner_rings.iter().flat_map(|ol| l_to_points(ol)));
        
        let mut newpoints = self.points.iter().flat_map(|p| {
            
            let mut nearest_other_line = other_lines
            .iter()
            .filter_map(|(start, end)| {
                let dst = dist_to_segment(*p, *start, *end);
                if dst.distance < maxdst {
                    Some(dst)
                } else {
                    None
                }
            })
            .map(|s| s.nearest_point)
            .collect::<Vec<_>>();

            nearest_other_line.sort_by(|a, b| a.dist(p).total_cmp(&b.dist(p)));

            let mut ret = vec![*p];
            ret.append(&mut nearest_other_line);
            ret
        }).collect::<Vec<_>>();

        newpoints.dedup_by(|a, b| a.equals(b));

        SvgLine {
            points: newpoints,
        }
    }

    pub fn reverse(&self) -> SvgLine {
        let mut p = self.points.clone();
        p.reverse();
        Self { points: p }
    }
    
    pub fn is_closed(&self) -> bool {
        self.is_closed_internal().is_some()
    }

    fn is_closed_internal(&self) -> Option<()> {
        let first = self.points.first()?;
        let last = self.points.last()?;
        if first.equals(last) {
            Some(())
        } else {
            None
        }
    }

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

    pub fn translate(&self, x: f64, y: f64) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
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


    pub fn equals_approx(&self, other: &Self, epsilon: f64) -> bool {
        approx_eq!(f64, self.x, other.x, epsilon = epsilon) &&
        approx_eq!(f64, self.y, other.y, epsilon = epsilon)
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitNasXml {
    #[serde(default = "default_etrs33")]
    pub crs: String,
    // Flurstücke, indexiert nach Flst ID, mit Nutzungen als Polygonen
    pub flurstuecke_nutzungen: BTreeMap<FlstId, Vec<TaggedPolygon>>,
}

impl Default for SplitNasXml {
    fn default() -> Self {
        Self {
            flurstuecke_nutzungen: BTreeMap::new(),
            crs: default_etrs33(),
        }
    }
}

fn default_etrs33() -> String {
    "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33".to_string()
}

impl SplitNasXml {

    pub fn get_linien_quadtree(&self) -> LinienQuadTree {

        let mut alle_linie_split_flurstuecke = self.flurstuecke_nutzungen.iter().flat_map(|(_, s)| {
            s.iter().flat_map(|q| {
                let mut lines = q.poly.outer_rings.iter().flat_map(crate::geograf::l_to_points).collect::<Vec<_>>();
                lines.extend(q.poly.inner_rings.iter().flat_map(crate::geograf::l_to_points));
                lines
            })
        }).collect::<Vec<_>>();
        alle_linie_split_flurstuecke.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        alle_linie_split_flurstuecke.dedup();
        let alle_linie_split_flurstuecke = alle_linie_split_flurstuecke;

        LinienQuadTree::new(alle_linie_split_flurstuecke)
    }

    pub fn get_flst_part_by_id(&self, flstpartid: &str) -> Option<&TaggedPolygon> {
        let split = flstpartid.split(":").collect::<Vec<_>>();
        let (ax_flurstueck, ax_ebene, cut_obj_id, intersect_id) = match &split[..] {
            &[a, e, o, i] => (a, e, o, Some(i)),
            &[a, e, o] => (a, e, o, None),
            _ => return None,
        };

        match intersect_id {
            Some(s) => {
                self.flurstuecke_nutzungen.get(ax_flurstueck)?
                .iter()
                .find(|p| {
                    p.attributes.get("AX_Ebene").map(|s| s.as_str()) == Some(ax_ebene) &&
                    p.attributes.get("AX_IntersectionId").map(|s| s.as_str()) == Some(s) &&
                    p.attributes.get("id").map(|s| s.as_str()) == Some(cut_obj_id)
                })
            },
            None => {
                self.flurstuecke_nutzungen.get(ax_flurstueck)?
                .iter()
                .find(|p| {
                    p.attributes.get("AX_Ebene").map(|s| s.as_str()) == Some(ax_ebene) &&
                    p.attributes.get("id").map(|s| s.as_str()) == Some(cut_obj_id)
                })
            }
        }
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
    pub fn from_aenderungen(aenderungen: &Aenderungen) -> Self {

        let original = NasXMLFile {
            crs: "".to_string(),
            ebenen: aenderungen.na_polygone_neu.iter().map(|(k, v)| {
                (k.clone(), vec![TaggedPolygon {
                    attributes: {
                        let mut q = vec![
                            ("aenderungID".to_string(), k.to_string()),
                        ];
                        if let Some(n) = v.nutzung.clone() {
                            q.push(("nutzung".to_string(), n.to_string()));
                        }
                        q.into_iter().collect()
                    },
                    poly: v.poly.clone(),
                }])
            }).collect::<BTreeMap<_, _>>()
        };
    
        let mut ebenen_map = BTreeMap::new();
        let mut items = BTreeMap::new();
        let mut itemid = 0;
        for (flst_id, polys) in original.ebenen.iter() {
            for (i, p) in polys.iter().enumerate() {
                let id = quadtree_f32::ItemId(itemid);
                itemid += 1;
                items.insert(id, quadtree_f32::Item::Rect(p.get_rect()));
                ebenen_map.insert(id, (flst_id.clone(), i));
            }
        }
        
        let qt = QuadTree::new(items.into_iter());

        Self {
            items: itemid + 1,
            original: original.clone(),
            qt: qt,
            ebenen_map,
        }
    }

    pub fn get_overlapping_flst(&self, rect: &quadtree_f32::Rect) -> Vec<TaggedPolygon> {
        self.qt.get_ids_that_overlap(rect)
        .into_iter()
        .filter_map(|itemid| {
            let (flst_id, i) = self.ebenen_map.get(&itemid)?;
            self.original.ebenen.get(flst_id)?.get(*i).cloned()
        }).collect()
    }

    // return = empty if points not on any flst line
    pub fn get_line_between_points(
        &self, 
        start: &SvgPoint, 
        end: &SvgPoint, 
        log: &mut Vec<String>, 
        maxdst_line: f64, 
        maxdst_line2: f64,
        maxdev_followline: f64,
        exclude_id: Option<String>,
    ) -> Vec<SvgPoint> {
        let mut polys = self.get_overlapping_flst(&start.get_rect(maxdst_line));
        polys.extend(self.get_overlapping_flst(&end.get_rect(maxdst_line)));
        polys.sort_by(|a, b| a.attributes.get("id").cmp(&b.attributes.get("id")));
        polys.dedup_by(|a, b| a.attributes.get("id") == b.attributes.get("id"));
        if let Some(eid) = exclude_id {
            polys.retain(|r| r.attributes.get("aenderungId").as_deref() != Some(&eid));
        }
        for p in polys {
            let v = p.get_line_between_points(start, end, log, maxdst_line2, maxdev_followline);
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
    pub original: SplitNasXml,
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
    pub fn get_overlapping_flst(&self, rect: &quadtree_f32::Rect) -> Vec<(String, TaggedPolygon)> {
        self.qt.get_ids_that_overlap(rect)
        .into_iter()
        .filter_map(|itemid| {
            let (flst_id, i) = self.flst_nutzungen_map.get(&itemid)?;
            Some((flst_id.clone(), self.original.flurstuecke_nutzungen.get(flst_id)?.get(*i).cloned()?))
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
        (ItemId(*k), Item::Rect(v.get_rect()))
    }));

    let flurstuecke_nutzungen = ax_flurstuecke.iter().filter_map(|flst| {

        let id = flst.attributes.get("flurstueckskennzeichen")?.replace("_", "");
        let id = FlstIdParsed::from_str(&id).parse_num()?.format_start_str();

        let bounds = flst.get_rect();
        let ids = nutzungs_qt.get_ids_that_overlap(&bounds);
        let polys = ids.iter().filter_map(|i| btree_id_to_poly.get(&i.0)).collect::<Vec<_>>();

        let flst_area = flst.poly.area_m2();

        let mut polys = polys.iter().flat_map(|p| {
            let intersection_mp = intersect_polys(&flst.poly, &p.poly);
            intersection_mp
            .into_iter()
            .filter(|p| !p.is_zero_area())
            .enumerate()
            .filter_map(|(i, svg_poly)| {

                let tp = TaggedPolygon {
                    attributes: {
                        let mut attrs = p.attributes.clone();
                        attrs.insert("AX_Flurstueck".to_string(), id.clone());
                        attrs.insert("AX_IntersectionId".to_string(), i.to_string());
                        attrs
                    },
                    poly: svg_poly,
                };
                let ebene = p.attributes.get("AX_Ebene")?;
                let kuerzel = tp.get_auto_kuerzel(&ebene)?;
                let nak = TaggedPolygon::get_nutzungsartenkennung(&kuerzel)?;
                Some((tp, nak))
            })
        }).collect::<Vec<_>>();

        polys.sort_by(|a, b| a.1.cmp(&b.1));
        let mut sum_poly_areas = 0.0;
        let mut final_polys = Vec::new();
        for tp in polys.iter() {
            let tp_area = tp.0.poly.area_m2();
            sum_poly_areas += tp_area;
            if sum_poly_areas < (flst_area + 1.0) {
                final_polys.push(tp.0.clone());
            }
        }

        if final_polys.is_empty() {
            None
        } else {
            Some((id.clone(), final_polys))
        }
    }).collect();

    Ok(SplitNasXml {
        crs: input.crs.clone(),
        flurstuecke_nutzungen,
    })
}

pub fn cleanup_poly(s: &SvgPolygon) -> SvgPolygon {
    let s = s.round_to_3dec();

    log_status("cleanup poly...");
    let outer_rings = s.outer_rings.iter()
    .filter_map(|l| if SvgPolygon::from_line(l).is_zero_area() { None } else { Some(l) })
    .filter_map(|r| {
        let mut s =  SvgPolygon::from_line(r);
        s.correct_winding_order();
        if s.is_zero_area()  { None } else { s.outer_rings.get(0).cloned() }
    })
    .flat_map(|r| clean_ring_2(&r))
    .collect();

    let inner_rings = s.inner_rings.iter()
    .filter_map(|l| if SvgPolygon::from_line(l).is_zero_area() { None } else { Some(l) })
    .filter_map(|r| {
        let mut s =  SvgPolygon::from_line(r);
        s.correct_winding_order();
        if s.is_zero_area()  { None } else { s.outer_rings.get(0).cloned() }
    })
    .flat_map(|r| clean_ring_2(&r))
    .map(|l| l.reverse())
    .collect();

    log_status("cleanup poly done...");

    SvgPolygon {
        outer_rings,
        inner_rings,
    }
}

fn clean_ring_2(r: &SvgLine) -> Vec<SvgLine> {
    
    let mut p1 = clean_points(&r.points);
    p1.reverse();
    let mut p2 = clean_points(&p1);
    p2.reverse();

    let mut v = Vec::new();
    clean_ring_selfintersection(&SvgLine { points: p2 }, &mut v);
    v
}

const CLEAN_LINE_DST: f64 = 0.1;

fn clean_points(points: &[SvgPoint]) -> Vec<SvgPoint> {
    // insert points whenever a line ends on another line
    let mut lines = points.windows(2).map(|a| match &a {
        &[a, b] => vec![*a, *b],
        _ => Vec::new(),
    }).collect::<Vec<_>>();

    for r in points.iter().skip(1).take(points.len().saturating_sub(2)) {
        for p in lines.iter_mut() {
            let start = match p.first() {
                Some(s) => s,
                None => continue,
            };
            let end = match p.last() {
                Some(s) => s,
                None => continue,
            };
            let dst = dist_to_segment(*r, *start, *end);
            if dst.distance < CLEAN_LINE_DST {
                *p = vec![*start, *r, *end];
            }
        }
    }

    let mut points = lines.into_iter().flat_map(|v| v.into_iter()).collect::<Vec<_>>();
    points.dedup_by(|a, b| a.equals(&b));
    points
}

fn clean_ring_selfintersection(line: &SvgLine, v: &mut Vec<SvgLine>) {
    let mut ranges_selfintersection = Vec::new();
    for (i, p) in line.points.iter().enumerate().skip(1) {
        for (q, r) in line.points.iter().enumerate().skip(i + 1) {
            if r.equals(p) {
                ranges_selfintersection.push((i + 1)..q);
            }
        }
    }
    ranges_selfintersection.retain(|r| !r.is_empty());
    ranges_selfintersection.retain(|r| *r != (0..line.points.len()));
    ranges_selfintersection.retain(|r| *r != (0..(line.points.len() - 1)));
    ranges_selfintersection.sort_by(|a, b| a.start.cmp(&b.start));
    ranges_selfintersection.dedup();

    // fix "bridge" polygons
    for r in ranges_selfintersection.iter() {
        let points = line.points[r.clone()].to_vec();
        let mut l = SvgLine { points: points };
        if !l.is_closed() {
            l.points.push(line.points[r.end]);
        }
        let poly = SvgPolygon::from_line(&line);
        log_status(&format!("bridge: {}", serde_json::to_string(&poly).unwrap_or_default()));
        if !poly.is_zero_area() {
            clean_ring_selfintersection(&l, v);
        }
    }

    if ranges_selfintersection.is_empty() {
        v.push(line.clone());
        return;
    }
    let mut newpoints = Vec::new();
    for (i, p) in line.points.iter().enumerate() {
        if ranges_selfintersection.iter().any(|r| r.contains(&i)) {
            continue;
        }
        newpoints.push(*p);
    }
    newpoints.dedup_by(|a, b| a.equals(&b));
    v.push(SvgLine {
        points: newpoints,
    });
}


macro_rules! define_func {($fn_name:ident, $op:expr) => {
        
    pub fn $fn_name(a: &SvgPolygon, b: &SvgPolygon) -> Vec<SvgPolygon> {
        use geo::BooleanOps;
        use crate::nas::EqualsAnyRingStatus::*;

        let mut a = a.round_to_3dec();
        let mut b = b.round_to_3dec();
        a.correct_winding_order();
        b.correct_winding_order();
        if a.is_zero_area() {
            return Vec::new();
        }
        if b.is_zero_area() {
            return Vec::new();
        }
        if a.equals(&b) {
            return vec![a];
        }
        let a = translate_to_geo_poly(&a);
        let b = translate_to_geo_poly(&b);
        let intersect = a.boolean_op(&b, $op);
        let mut s = translate_from_geo_poly(&intersect);
        if $op == geo::OpType::Xor {
            log_status(&serde_json::to_string(&s).unwrap_or_default());
        }
        for q in s.iter_mut() {
            q.correct_winding_order();
        }
        s
    }
};}

define_func!(intersect_polys, geo::OpType::Intersection);
define_func!(xor_polys, geo::OpType::Xor);

fn xor_combine(a: &SvgPolygon, b: &SvgPolygon) -> SvgPolygon {
    let mut aor = a.outer_rings.clone();
    let mut air = a.inner_rings.clone();
    aor.extend(b.outer_rings.iter().cloned());
    air.extend(b.inner_rings.iter().cloned());
    SvgPolygon {
        outer_rings: aor,
        inner_rings: air,
    }
}

fn union(a: &SvgPolygon, b: &SvgPolygon) -> Vec<SvgPolygon> {
    let xor = xor_combine(a, b);
    translate_from_geo_poly(&geo::MultiPolygon(vec![translate_to_geo_poly(&xor).convex_hull()]))
}

pub fn convex_hull_polys(a: &SvgPolygon, b: &[SvgPolygon]) -> SvgPolygon {
    let mut x = a.clone();
    for b in b.iter() {
        x = xor_combine(&x, b);
    }
    translate_from_geo_poly(&geo::MultiPolygon(vec![translate_to_geo_poly(&x).convex_hull()]))
    .get(0).cloned().unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgPolyInternalResult {
    pub num_points: usize,
    pub points_touching_lines: usize,
    pub points_inside_other_poly: usize,
    pub all_points_are_on_line: bool,
}

impl SvgPolyInternalResult {
    pub fn overlaps_other_poly(&self) -> bool {
        self.points_inside_other_poly != 0
    }

    pub fn is_contained_in_other_poly(&self) -> bool {
        self.points_touching_lines + self.points_inside_other_poly >= self.num_points
    }
}

fn point_in_line(p: &SvgPoint, l: &SvgLine) -> bool {
    if point_in_line_2(p, l) {
        return true;
    }
    let mut l = l.clone();
    l.points.reverse();
    point_in_line_2(p, &l)
}

fn point_in_line_2(p: &SvgPoint, l: &SvgLine) -> bool {
    let mut count = 0;
    for side in l.points.windows(2) {
        match &side {
            &[a, b] => if ray_intersects_segment(p, (a, b)) {
                count += 1;
            },
            _ => { }
        }
    }
    if count % 2 == 0 {
        false // outside
    } else {
        true // inside 
    }
}

fn ray_intersects_segment(p: &SvgPoint, (mut a, mut b): (&SvgPoint, &SvgPoint)) -> bool {

    // B must be "above" A
    if b.y < a.y {
        std::mem::swap(&mut a, &mut b);
    }

    let mut p: SvgPoint = p.clone();

    let eps = 0.001;
    if p.y == a.y || p.y == b.y {
        p.y = p.y + eps;
    }

    if p.y < a.y || p.y > b.y {
        return false; // out of bounds
    } else if p.x >= a.x.max(b.x) {
        return false; // out of bounds
    }

    if p.x < a.x.min(b.x) {
        return true;
    }

    let m_red = if a.x != b.x {
        (b.y - a.y)/(b.x - a.x)
    } else {
        std::f64::INFINITY
    };

    let m_blue = if a.x != p.x {
        (p.y - a.y)/(p.x - a.x)
    } else {
        std::f64::INFINITY
    };
    
    m_blue >= m_red
}

pub fn point_is_in_polygon(p: &SvgPoint, poly: &SvgPolygon) -> bool {
    
    let mut c_in_outer = false;
    for o in poly.outer_rings.iter() {
        if point_in_line(p, o) {
            c_in_outer = true;
            break;
        }
    }

    if c_in_outer {
        for i in poly.inner_rings.iter() {
            if point_in_line(p, i) {
                c_in_outer = false;
                break;
            }
        }
    }

    c_in_outer
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relate {
    pub is_1: SvgPolyInternalResult,
    pub is_2: SvgPolyInternalResult,
}

impl Relate {

    pub fn touches_other_poly_outside(&self) -> bool {
        (self.is_1.points_inside_other_poly == 0 && self.is_2.points_inside_other_poly == 0) &&
        (self.is_1.points_touching_lines != 0 || self.is_2.points_touching_lines != 0)
    }

    pub fn only_touches(&self) -> bool {
        // no intersection of the two polygons possible
        if self.is_1.points_inside_other_poly == 0 && self.is_2.points_inside_other_poly == 0 {
            if self.is_1.all_points_are_on_line || self.is_2.all_points_are_on_line {
                // a is a subset of b or b is a subset of a
                false
            } else {
                true
            }
        } else {
            false
        }
    }

    pub fn overlaps(&self) -> bool {
        self.is_1.overlaps_other_poly() ||
        self.is_2.overlaps_other_poly()
    }

    pub fn a_contained_in_b(&self) -> bool {
        self.is_1.is_contained_in_other_poly()
    }

    pub fn b_contained_in_a(&self) -> bool {
        self.is_2.is_contained_in_other_poly()
    }
}

pub fn relate(a: &SvgPolygon, b: &SvgPolygon, dst: f64) -> Relate {
    let is_1 = only_touches_internal(a, b, dst);
    let is_2 = only_touches_internal(b, a, dst);
    Relate {
        is_1,
        is_2,
    }
}

pub fn line_contained_in_line(outer: &SvgLine, inner: &SvgLine) -> bool {
    for p in inner.points.iter() {
        if !point_in_line(p, outer) {
            return false;
        }
    }
    true
}

// Only touches the other polygon but does not intersect
pub fn only_touches_internal(a: &SvgPolygon, b: &SvgPolygon, dst: f64) -> SvgPolyInternalResult {

    let points_a = a.outer_rings.iter().flat_map(|l| l.points.iter()).collect::<Vec<_>>();
    // let b_geo = translate_to_geo_poly(b);

    let mut points_touching_lines = 0;
    let mut points_inside_other_poly = 0;
    let mut all_points_are_on_line = true;
    for start_a in points_a.iter() {
        if point_is_on_any_line(start_a, &b, dst) {
            points_touching_lines += 1;
        } else if point_is_in_polygon(*start_a, &b) {
            points_inside_other_poly += 1;
            all_points_are_on_line = false;
        } else {
            all_points_are_on_line = false;
        }
    }

    SvgPolyInternalResult {
        num_points: points_a.len(),
        points_touching_lines,
        points_inside_other_poly,
        all_points_are_on_line
    }
}

pub fn point_is_on_any_line(p: &SvgPoint, poly: &SvgPolygon, dst: f64) -> bool {
    for line in poly.outer_rings.iter() {
        for q in line.points.windows(2) {
            match &q {
                &[sa, eb] => {
                    if dist_to_segment(*p, *sa, *eb).distance < dst {
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

pub fn translate_geoline(a: &SvgLine) -> geo::LineString<f64> {
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

pub fn translate_from_geo_poly_special(a: &geo::MultiPolygon<f64>) -> Vec<SvgPolygon> {
    a.0.iter().flat_map(|s| {
        let mut q = vec![translate_ring(s.exterior())];
        q.extend(s.interiors().iter().map(translate_ring));
        q.iter().map(|l| {
            let mut p = SvgPolygon::from_line(l); 
            p.correct_winding_order();
            p
        }).collect::<Vec<_>>()
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