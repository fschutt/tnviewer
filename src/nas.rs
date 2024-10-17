use crate::{
    csv::CsvDataType, david::log_aenderungen, geograf::{
        points_to_rect,
        LinienQuadTree,
    }, ui::{
        dist_to_segment,
        Aenderungen,
        AenderungenIntersection,
    }, uuid_wasm::{log_status, log_status_clear, uuid}, xlsx::FlstIdParsed, xml::{
        get_all_nodes_in_subtree,
        XmlNode,
    }
};
use chrono::DateTime;
use float_cmp::approx_eq;
use geo::{
    Area,
    Centroid,
    CoordsIter,
    TriangulateEarcut,
    Within,
};
use proj4rs::Proj;
use quadtree_f32::{
    Item,
    ItemId,
    QuadTree,
    Rect,
};
use serde_derive::{
    Deserialize,
    Serialize,
};
use serde::{Deserialize, Deserializer};
use std::collections::{
    BTreeMap,
    BTreeSet,
};

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

    pub fn get_de_id_in_rect(&self, r: &quadtree_f32::Rect) -> Vec<String> {
        let ebenen = crate::get_nutzungsartenkatalog_ebenen().into_values().collect::<BTreeSet<_>>();
        self.ebenen.iter().flat_map(|s| {
            if !ebenen.contains(s.0) {
                return Vec::new();
            }
            s.1.iter().filter_map(|q| {
                if q.poly.get_rect().overlaps_rect(r) {
                    let kuerzel = q.get_auto_kuerzel()?;
                    let m2 = q.poly.area_m2().round();
                    q.attributes.get("id").cloned().map(|s| { format!("{m2} m2 {kuerzel} ({s})")})
                } else {
                    None
                }
            }).collect()
        }).collect()
    }

    pub fn fortfuehren(&self, aenderungen: &Aenderungen, split_nas: &SplitNasXml) -> Self {

        use crate::david::Operation::*;

        // let aenderungen_todo = crate::david::get_aenderungen_internal(aenderungen, &self, split_nas);
        let aenderungen_todo = crate::david::get_aenderungen_internal_definiert_only(
            aenderungen, 
            &self, 
            split_nas
        );

        /*
        let aenderungen_todo = crate::david::merge_aenderungen_with_existing_nas(
            &aenderungen_todo,
            &self,
        );
        */

        let aenderungen_todo = crate::david::insert_gebaeude_delete(
            &aenderungen,
            &aenderungen_todo,
        );

        log_status_clear();
        log_status("NasXMLFile::fortfuehren");
        log_aenderungen(&aenderungen_todo);
        log_status("----");

        let objs_to_delete = aenderungen_todo.iter().filter_map(|s| {
            match s {
                Delete { obj_id, .. } => Some(obj_id.clone()),
                _ => None,
            }
        }).collect::<BTreeSet<_>>();

        let ebenen = self.ebenen.iter().map(|(k, v)| {
            (k.clone(), v.iter().filter_map(|q| {
                if let Some(s) = q.attributes.get("id") {
                    if objs_to_delete.contains(s) { None } else { Some(q) }
                } else {
                    Some(q)
                }
            }).collect::<Vec<_>>())
        }).collect::<BTreeMap<_, _>>();

        let objs_to_replace = aenderungen_todo.iter().filter_map(|s| {
            match s {
                Replace { obj_id, poly_neu, .. } => Some((obj_id.clone(), poly_neu)),
                _ => None,
            }
        }).collect::<BTreeMap<_, _>>();

        let mut ebenen = ebenen.iter().map(|(k, v)| {
            (k.clone(), v.iter().filter_map(|q| {
                if let Some(s) = q.attributes.get("id") {
                    if let Some(repl) = objs_to_replace.get(s) {
                        Some(TaggedPolygon { poly: (*repl).clone(), attributes: q.attributes.clone() })
                    } else { 
                        Some((*q).clone()) 
                    }
                } else {
                    Some((*q).clone())
                }
            }).collect::<Vec<_>>())
        }).collect::<BTreeMap<_, _>>();

        for (id, a) in aenderungen_todo.iter().enumerate() {
            match a {
                Insert { ebene, kuerzel, poly_neu } => {
                    let id = ("DE_001".to_string() + &format!("{id:010}"));
                    let extra_attr = vec![
                        ("AX_Ebene", ebene.as_str()),
                        ("id", id.as_str()),
                    ];
                    let tp = TaggedPolygon {
                        poly: poly_neu.clone(),
                        attributes: TaggedPolygon::get_auto_attributes_for_kuerzel(&kuerzel, &extra_attr),
                    };
                    
                    ebenen
                    .entry(ebene.clone())
                    .or_insert_with(|| Vec::new())
                    .push(tp);
                },
                _ => { },
            }
        }

        Self {
            crs: self.crs.clone(),
            ebenen: ebenen
        }
    }

    pub fn get_linien_quadtree(&self) -> LinienQuadTree {
        let default = Vec::new();
        let mut alle_linie_split_flurstuecke = self
            .ebenen
            .get("AX_Flurstueck")
            .unwrap_or(&default)
            .iter()
            .flat_map(|q| {
                let mut lines = crate::geograf::l_to_points(&q.poly.outer_ring);
                lines.extend(
                    q.poly
                        .inner_rings
                        .iter()
                        .flat_map(crate::geograf::l_to_points),
                );
                lines
            })
            .collect::<Vec<_>>();
        alle_linie_split_flurstuecke.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
        alle_linie_split_flurstuecke.dedup();
        let alle_linie_split_flurstuecke = alle_linie_split_flurstuecke;

        LinienQuadTree::new(alle_linie_split_flurstuecke)
    }

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
    pub fn get_gebaeude(&self, _csv: &CsvDataType, aenderungen: &Aenderungen) -> String {
        let ax_flurstuecke = match self.ebenen.get("AX_Flurstueck") {
            Some(o) => o,
            None => return format!("keine Ebene AX_Flurstueck vorhanden"),
        };

        let ax_gebaeude = match self.ebenen.get("AX_Gebaeude") {
            Some(o) => o,
            None => return format!("keine Ebene AX_Gebaeude vorhanden"),
        };

        // Flurstueck_ID => Flurstueck Poly
        let ax_flurstuecke_map = ax_flurstuecke
            .iter()
            .filter_map(|tp| {
                let flst_id = tp.attributes.get("flurstueckskennzeichen").cloned()?;
                let rect = tp.get_rect();
                Some((flst_id, rect, &tp.poly))
            })
            .collect::<Vec<(_, _, _)>>();

        let gebaeude_avail = ax_gebaeude
            .iter()
            .enumerate()
            .filter_map(|(_i, tp)| {
                let gebaeude_id = tp.attributes.get("id").cloned()?;
                let flst_rect = tp.get_rect();
                let flst = ax_flurstuecke_map
                    .iter()
                    .filter(|(_id, r, _poly)| flst_rect.overlaps_rect(r))
                    .filter(|(_id, _r, poly)| {
                        let relate = crate::nas::relate(poly, &tp.poly, 1.0);
                        relate.overlaps()
                        || relate.a_contained_in_b()
                        || relate.b_contained_in_a()
                    })
                    .map(|(id, _, _)| id.clone())
                    .collect::<Vec<_>>();

                Some((
                    gebaeude_id.clone(),
                    GebaeudeInfo {
                        flst_id: flst.clone(),
                        deleted: aenderungen
                            .gebaeude_loeschen
                            .values()
                            .any(|v| v.gebaeude_id == gebaeude_id),
                        gebaeude_id: gebaeude_id.clone(),
                        poly: tp.clone(),
                    },
                ))
            })
            .collect::<BTreeMap<_, _>>();

        let geom = gebaeude_avail.iter().map(|(_k, v)| {

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

            let poly = convert_poly_to_string(&v.poly.poly.outer_ring, &holes);
            format!("{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"Polygon\", \"coordinates\": {poly} }} }}")
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
            let flst = o
                .attributes
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
                id: o
                    .attributes
                    .get("flurstueckskennzeichen")
                    .cloned()
                    .unwrap_or_default(),
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
    let geom = objekte.iter().map(|poly| {

        let holes = poly.poly.inner_rings.iter()
        .map(convert_svgline_to_string)
        .collect::<Vec<_>>()
        .join(",");

        let feature_map = poly.attributes
        .iter().map(|(k, v)| format!("{k:?}: {v:?}"))
        .collect::<Vec<_>>().join(",");

        let poly = convert_poly_to_string(&poly.poly.outer_ring, &holes);
        format!("{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"Polygon\", \"coordinates\": {poly} }} }}")
    }).collect::<Vec<_>>().join(",");

    format!("{{ \"type\": \"FeatureCollection\", \"features\": [{geom}] }}")
}

fn convert_poly_to_string(p: &SvgLine, holes: &str) -> String {
    format!(
        "[{src}{comma}{holes}]",
        src = convert_svgline_to_string(p),
        comma = if holes.trim().is_empty() { "" } else { "," },
        holes = holes,
    )
}

fn convert_svgline_to_string(q: &SvgLine) -> String {
    format!(
        "[{}]",
        q.points
            .iter()
            .map(|s| format!("[{}, {}]", s.x, s.y))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TaggedPolygon {
    pub poly: SvgPolygonInner,
    pub attributes: BTreeMap<String, String>,
}

impl TaggedPolygon {
    // returns "DEBBAL730002acBQ" for example
    pub fn get_object_id(s: &str) -> Option<String> {
        s.split(":").nth(2).map(|s| s.to_string())
    }

    pub fn get_auto_kuerzel(&self) -> Option<String> {
        let nak = crate::get_nutzungsartenkatalog();
        nak.iter()
            .filter_map(|(kuerzel, na)| {
                let atr = na.atr.split(",").filter_map(|s| {
                    let mut sq = s.trim().split("=");
                    let key = sq.next()?;
                    let value = sq.next()?;
                    Some((key, value))
                });
                for (k, v) in atr {
                    if self.attributes.get(k).map(|s| s.as_str()) != Some(v) {
                        return None;
                    }
                }
                Some((kuerzel, &na.atr))
            })
            .max_by_key(|a| a.1.len())
            .map(|s| s.0.clone())
    }

    pub fn get_auto_ebene(kuerzel: &str) -> Option<String> {
        let nak = crate::get_nutzungsartenkatalog();
        nak.get(kuerzel)?.atr.split(",").find_map(|kv| {
            let mut sp = kv.split("=");
            let k = sp.next()?;
            let v = sp.next()?;
            if k == "AX_Ebene" {
                Some(v.to_string())
            } else {
                None
            }
        })
    }

    pub fn get_auto_attributes_for_kuerzel(
        kuerzel: &str,
        extra: &[(&str, &str)],
    ) -> BTreeMap<String, String> {
        let nak = crate::get_nutzungsartenkatalog();
        let attribute = nak.get(kuerzel).map(|s| s.atr.as_str()).unwrap_or("");
        let mut map = BTreeMap::new();
        for kv in attribute.split(",") {
            let mut sp = kv.split("=");
            let k = sp.next();
            let v = sp.next();
            if let (Some(k), Some(v)) = (k, v) {
                map.insert(k.to_string(), v.to_string());
            }
        }
        for (k, v) in extra {
            map.insert(k.to_string(), v.to_string());
        }
        map
    }

    pub fn get_ebene(&self) -> Option<String> {
        self.attributes.get("AX_Ebene").cloned()
    }

    pub fn get_de_id(&self) -> Option<String> {
        self.attributes.get("id").cloned()
    }

    pub fn get_intersection_id(&self) -> Option<String> {
        self.attributes.get("AX_IntersectionId").cloned()
    }

    pub fn get_flurstueck_id(&self) -> Option<String> {
        self.attributes.get("AX_Flurstueck").cloned()
    }

    pub fn get_flst_part_id(&self) -> Option<String> {
        let ebene = self.get_ebene()?;
        let id = self.get_de_id()?;
        let intersection = self
            .get_intersection_id()
            .unwrap_or_else(|| "0".to_string());
        let flurstueck = self.get_flurstueck_id()?;
        Some(format!("{flurstueck}:{ebene}:{id}:{intersection}"))
    }

    fn check_line_for_points(
        l: &SvgLine,
        start: &SvgPoint,
        end: &SvgPoint,
        dst: f64,
        maxdev_followline: f64,
    ) -> Vec<SvgPoint> {
        let start = start.round_to_3dec();
        let end = end.round_to_3dec();

        let mut start_is_on_line = None;
        let mut end_is_on_line = None;

        let mut pos_start = match l.points.iter().position(|p| p.equals(&start)) {
            Some(s) => s,
            None => {
                let starting_point_on_lines = l
                    .points
                    .iter()
                    .enumerate()
                    .zip(l.points.iter().skip(1))
                    .map(|((pos, s0), e0)| {
                        (
                            pos,
                            s0.clone(),
                            e0.clone(),
                            crate::ui::dist_to_segment(start, *s0, *e0),
                        )
                    })
                    .collect::<Vec<_>>();

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
            }
        };

        let mut pos_end = match l.points.iter().position(|p| p.equals(&end)) {
            Some(s) => s,
            None => {
                let ending_point_on_lines = l
                    .points
                    .iter()
                    .enumerate()
                    .zip(l.points.iter().skip(1))
                    .map(|((pos, s0), e0)| {
                        let p_is_on_line = crate::ui::dist_to_segment(end, *s0, *e0);
                        (pos, s0.clone(), e0.clone(), p_is_on_line)
                    })
                    .collect::<Vec<_>>();

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
            }
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

        let normal = l
            .points
            .iter()
            .skip(pos_start.saturating_add(1))
            .take(normal_direction.saturating_sub(1))
            .cloned()
            .collect::<Vec<_>>();

        let mut rev = l
            .points
            .iter()
            .skip(pos_end.saturating_add(1))
            .cloned()
            .collect::<Vec<_>>();
        rev.extend(l.points.iter().cloned().take(pos_start));
        rev.reverse();

        let normal_error = normal
            .iter()
            .map(|s| dist_to_segment(*s, start, end).distance.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let reverse_error = rev
            .iter()
            .map(|s| dist_to_segment(*s, start, end).distance.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let reverse = reverse_error < normal_error;

        let mut ret = if reverse { rev } else { normal };

        ret.dedup_by(|a, b| a.equals(b));

        if pos_end == pos_start {
            return Vec::new();
        }

        if ret.len() > 1 {
            match (l.points.get(pos_start), l.points.get(pos_end)) {
                (Some(l_start), Some(l_end)) => {
                    if l_start.equals(l_end) {
                        return Vec::new();
                    }

                    let mut line_normal = vec![*l_start];
                    for p in ret.iter() {
                        line_normal.push(*p);
                    }
                    line_normal.push(*l_end);
                    let line_normal_length = line_normal
                        .windows(2)
                        .map(|pts| match &pts {
                            &[a, b] => a.dist(b),
                            _ => 0.0,
                        })
                        .sum::<f64>();

                    let mut line_reverse = vec![*l_start];
                    for p in ret.iter().rev() {
                        line_reverse.push(*p);
                    }
                    line_reverse.push(*l_end);
                    let line_reverse_length = line_reverse
                        .windows(2)
                        .map(|pts| match &pts {
                            &[a, b] => a.dist(b),
                            _ => 0.0,
                        })
                        .sum::<f64>();

                    if startend_swapped {
                        if line_normal_length < line_reverse_length {
                            ret.reverse();
                        }
                    } else {
                        if line_reverse_length < line_normal_length {
                            ret.reverse();
                        }
                    }
                }
                _ => {}
            }
        }

        let len_original = start.dist(&end);
        let mut len_merged_points = vec![start];
        len_merged_points.extend(ret.iter().cloned());
        len_merged_points.push(end);
        let len_merged = len_merged_points
            .windows(2)
            .map(|w| match &w {
                &[a, b] => a.dist(b),
                _ => 0.0,
            })
            .sum::<f64>();

        ret.retain(|p| !p.equals(&start) && !p.equals(&end));

        if len_original + maxdev_followline > len_merged {
            ret
        } else {
            Vec::new()
        }
    }

    fn check_lines_for_points(
        l: &[&SvgLine],
        start: &SvgPoint,
        end: &SvgPoint,
        dst: f64,
        maxdev_followline: f64,
    ) -> Vec<SvgPoint> {
        for l in l {
            let v = Self::check_line_for_points(l, start, end, dst, maxdev_followline);
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
        maxdst_line: f64,
        maxdev_followline: f64,
    ) -> Vec<SvgPoint> {
        let v = Self::check_lines_for_points(
            &[&self.poly.outer_ring],
            start,
            end,
            maxdst_line,
            maxdev_followline,
        );
        if !v.is_empty() {
            return v;
        }
        let v = Self::check_lines_for_points(
            &[&self.poly.outer_ring],
            start,
            end,
            maxdst_line,
            maxdev_followline,
        );
        if !v.is_empty() {
            return v;
        }
        Vec::new()
    }

    pub fn get_groesse(&self) -> f64 {
        translate_to_geo_poly_special_shared(&[&self.poly])
            .0
            .iter()
            .map(|p| p.signed_area())
            .sum()
    }

    pub fn get_wirtschaftsart(kuerzel: &str) -> Option<String> {
        let map = crate::get_nutzungsartenkatalog();
        map.get(kuerzel.trim()).map(|s| s.wia.clone())
    }

    pub fn get_nutzungsartenkennung(kuerzel: &str) -> Option<usize> {
        let map = crate::get_nutzungsartenkatalog();
        map.get(kuerzel.trim())
            .and_then(|s| s.nak.parse::<usize>().ok())
    }

    pub fn get_rect(&self) -> quadtree_f32::Rect {
        self.poly.get_rect()
    }

    pub fn get_fit_bounds(&self) -> [[f64; 2]; 2] {
        self.poly.get_fit_bounds()
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SvgPolygon {
    Old(SvgPolygonInner),
    New(SvgPolygonSerialize),
}

impl Default for SvgPolygon {
    fn default() -> Self {
        Self::Old(SvgPolygonInner::default())
    }
}

impl SvgPolygon {
    pub fn get_rect(&self) -> quadtree_f32::Rect {
        match self {
            Self::New(n) => n.get_old().get_rect(),
            Self::Old(n) => n.get_rect(),
        }
    }
    pub fn get_inner(&self) -> SvgPolygonInner {
        match self {
            Self::New(n) => n.get_old(),
            Self::Old(n) => n.clone(),
        }
    }

    pub fn migrate(&self) -> Self {
        match self {
            Self::New(n) => Self::New(n.clone()),
            Self::Old(n) => Self::New(n.ser()),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SvgPolygonInner {
    #[serde(alias = "outer_rings", deserialize_with = "linevec_deserialize")]
    pub outer_ring: SvgLine,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inner_rings: Vec<SvgLine>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SvgLineOrVec {
    Old(Vec<SvgLine>),
    New(SvgLine),
}

fn linevec_deserialize<'de, D: Deserializer<'de>>(f: D) -> Result<SvgLine, D::Error> {
    let s = SvgLineOrVec::deserialize(f)?;
    match s {
        SvgLineOrVec::Old(o) => Ok(o.get(0).cloned().unwrap_or_default()),
        SvgLineOrVec::New(o) => Ok(o),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SvgLineOrVecString {
    Old(Vec<String>),
    New(String),
}

fn linevec_deserialize_string<'de, D: Deserializer<'de>>(f: D) -> Result<String, D::Error> {
    let s = SvgLineOrVecString::deserialize(f)?;
    match s {
        SvgLineOrVecString::Old(o) => Ok(o.get(0).cloned().unwrap_or_default()),
        SvgLineOrVecString::New(o) => Ok(o),
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SvgPolygonSerialize {
    #[serde(deserialize_with = "linevec_deserialize_string")]
    pub or: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ir: Vec<String>,
}

impl SvgPolygonSerialize {
    pub fn get_old(&self) -> SvgPolygonInner {
        SvgPolygonInner {
            outer_ring: Self::line_from(&self.or.trim()),
            inner_rings: self
                .ir
                .iter()
                .map(|s| s.trim())
                .map(|q| Self::line_from(q))
                .collect(),
        }
    }

    fn line_from(s: &str) -> SvgLine {
        let p = s
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<_>>();
        let v = p
            .chunks(2)
            .filter_map(|s| match s {
                &[x, y] => Some(SvgPoint { x, y }),
                _ => None,
            })
            .collect::<Vec<_>>();
        SvgLine { points: v }
    }
}

impl SvgPolygonInner {
    fn ser(&self) -> SvgPolygonSerialize {
        SvgPolygonSerialize {
            or: Self::serialize_line(&self.outer_ring),
            ir: self
                .inner_rings
                .iter()
                .map(|s| Self::serialize_line(s))
                .collect::<Vec<_>>(),
        }
    }
    fn serialize_line(l: &SvgLine) -> String {
        l.points
            .iter()
            .map(|s| format!("{} {}", s.x, s.y))
            .collect::<Vec<_>>()
            .join(" ")
    }
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

impl SvgPolygonInner {

    pub fn overlaps(&self, other: &Self) -> bool {
        let self_rect = self.get_rect();
        let other_rect = other.get_rect();
        if !(self_rect.overlaps_rect(&other_rect) || other_rect.overlaps_rect(&self_rect)) {
            return false;
        }

        for p in self.outer_ring.points.iter() {
            if point_is_in_polygon(p, other) {
                return true;
            }
        }

        let triangle_points = translate_to_geo_poly_special_shared(&[self])
            .0
            .iter()
            .flat_map(|f| f.earcut_triangles())
            .map(|i| i.centroid())
            .map(|p| SvgPoint { x: p.x(), y: p.y() })
            .collect::<Vec<_>>();

        triangle_points
            .iter()
            .any(|p| point_is_in_polygon(p, other))
    }

    pub fn get_triangle_points(&self) -> Vec<SvgPoint> {
        translate_to_geo_poly_special_shared(&[self])
            .0
            .iter()
            .flat_map(|f| f.earcut_triangles())
            .filter_map(|f| {
                if f.unsigned_area() > 10.0 {
                    Some(f.centroid())
                } else {
                    None
                }
            })
            .map(|p| SvgPoint { x: p.x(), y: p.y() })
            .collect::<Vec<_>>()
    }

    pub fn is_completely_inside_of(&self, other: &Self) -> bool {
        let triangle_points = translate_to_geo_poly_special_shared(&[self])
            .0
            .iter()
            .flat_map(|f| f.earcut_triangles())
            .map(|i| i.centroid())
            .collect::<Vec<_>>();

        let other = translate_to_geo_poly_special_shared(&[other]);

        triangle_points.iter().all(|p| p.is_within(&other))
    }

    pub fn from_line(l: &SvgLine) -> Self {
        Self {
            outer_ring: l.clone(),
            inner_rings: Vec::new(),
        }
    }

    pub fn contains_polygon(&self, other: &Self) -> bool {
        for p in other.outer_ring.points.iter() {
            if !point_is_in_polygon(p, self) {
                return false;
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

    pub fn get_hash(&self) -> u64 {
        use highway::{
            HighwayHash,
            HighwayHasher,
        };
        let rounded = self.round_to_3dec().get_all_pointcoords_sorted();
        let bytes = rounded
            .iter()
            .flat_map(|[a, b]| {
                let mut a = a.to_le_bytes().to_vec();
                a.extend(b.to_le_bytes().into_iter());
                a
            })
            .collect::<Vec<_>>();
        HighwayHasher::default().hash64(&bytes)
    }


    pub fn get_all_points(&self) -> Vec<SvgPoint> {
        let mut v = Vec::new();
        for p in self.outer_ring.points.iter() {
            v.push(*p);
        }
        for l in self.inner_rings.iter() {
            for p in l.points.iter() {
                v.push(*p);
            }
        }
        v
    }

    pub fn get_all_pointcoords_sorted(&self) -> Vec<[usize; 2]> {
        let mut v = BTreeSet::new();
        for p in self.outer_ring.points.iter() {
            v.insert([
                (p.x * 1000.0).round() as usize,
                (p.y * 1000.0).round() as usize,
            ]);
        }
        for l in self.inner_rings.iter() {
            for p in l.points.iter() {
                v.insert([
                    (p.x * 1000.0).round() as usize,
                    (p.y * 1000.0).round() as usize,
                ]);
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

    pub fn get_fit_bounds(&self) -> [[f64; 2]; 2] {
        let mut min_x = self.outer_ring.points.get(0)
            .map(|p| p.x)
            .unwrap_or(0.0);
        let mut max_x = self.outer_ring.points.get(0)
            .map(|p| p.x)
            .unwrap_or(0.0);
        let mut min_y = self.outer_ring.points.get(0)
            .map(|p| p.y)
            .unwrap_or(0.0);
        let mut max_y = self.outer_ring.points.get(0)
            .map(|p| p.y)
            .unwrap_or(0.0);

        for p in self.outer_ring.points.iter() {
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

        [[min_y, min_x], [max_y, max_x]]
    }

    pub fn insert_points_from(&mut self, other: &Self, maxdst: f64) {
        self.outer_ring = self.outer_ring.insert_points_from(other, maxdst);
        self.inner_rings = self
            .inner_rings
            .iter()
            .map(|o| o.insert_points_from(other, maxdst))
            .collect();
        self.correct_almost_touching_points(other, maxdst * 2.0, false);
    }

    pub fn correct_almost_touching_points(
        &mut self,
        other: &Self,
        maxdst: f64,
        correct_points_on_lines: bool,
    ) {
        let mut other_points = Vec::new();
        let mut other_lines = Vec::new();

        for p in other.outer_ring.points.iter() {
            other_points.push(*p);
        }

        if correct_points_on_lines {
            for p in other.outer_ring.points.windows(2) {
                match p {
                    &[a, b] => other_lines.push((a, b)),
                    _ => {}
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
                        _ => {}
                    }
                }
            }
        }

        let max_items_points = other_points.len().saturating_div(20).max(500);
        let max_items_lines = other_lines.len().saturating_div(20).max(500);

        let qt_points = quadtree_f32::QuadTree::new_with_max_items_per_quad(
            other_points.iter().enumerate().map(|(i, s)| {
                (
                    ItemId(i),
                    Item::Point(quadtree_f32::Point { x: s.x, y: s.y }),
                )
            }),
            max_items_points,
        );

        for p in self.outer_ring.points.iter_mut() {
            let mut closest_other_point = qt_points
                .get_points_contained_by(&p.get_rect(maxdst))
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

        for l in self.inner_rings.iter_mut() {
            for p in l.points.iter_mut() {
                let mut closest_other_point = qt_points
                    .get_points_contained_by(&p.get_rect(maxdst))
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
            other_lines
                .iter()
                .enumerate()
                .map(|(i, s)| (ItemId(i), Item::Rect(points_to_rect(&(s.0, s.1))))),
            max_items_lines,
        );

        for p in self.outer_ring.points.iter_mut() {
            let mut closest_other_lines = qt_lines
                .get_ids_that_overlap(&p.get_rect(maxdst))
                .into_iter()
                .filter_map(|i| other_lines.get(i.0))
                .map(|q| dist_to_segment(*p, q.0, q.1))
                .filter(|s| s.distance < maxdst)
                .collect::<Vec<_>>();
            closest_other_lines.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            if let Some(first) = closest_other_lines.first() {
                *p = first.nearest_point;
            }
        }

        for l in self.inner_rings.iter_mut() {
            for p in l.points.iter_mut() {
                let mut closest_other_lines = qt_lines
                    .get_ids_that_overlap(&p.get_rect(maxdst))
                    .into_iter()
                    .filter_map(|i| other_lines.get(i.0))
                    .map(|q| dist_to_segment(*p, q.0, q.1))
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
            outer_ring: self.outer_ring.inverse_point_order(),
            inner_rings: self
                .inner_rings
                .iter()
                .map(|s| s.inverse_point_order())
                .collect(),
        }
    }

    pub fn correct_winding_order_cloned(&self) -> Self {
        let mut s = self.clone();
        s.correct_winding_order();
        s
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
        let area_m2 = self.area_m2();
        let reverse = self.inverse_point_order().area_m2();
        area_m2 < 1.0 && reverse < 1.0
    }

    pub fn area_m2(&self) -> f64 {
        crate::nas::translate_to_geo_poly_special_shared(&[self])
            .0
            .iter()
            .map(|p| p.signed_area())
            .sum::<f64>()
    }

    pub fn equals_any_ring(&self, other: &Self) -> EqualsAnyRingStatus {
        let first_ring = &self.outer_ring;
        let or = &other.outer_ring;

        if or.equals(first_ring) {
            return EqualsAnyRingStatus::EqualToRing(0);
        }

        let points_outside = Self::is_center_inside(first_ring, or);
        let points_inside = Self::is_center_inside(first_ring, or);
        let any_points_on_line = Self::any_points_equal(first_ring, or);

        if any_points_on_line {
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

    pub fn any_point_outside(a: &SvgLine, b: &SvgLine) -> bool {
        let tr = translate_to_geo_poly_special_shared(&[&SvgPolygonInner::from_line(a)]);
        let a_poly = match tr.0.get(0) {
            Some(s) => s,
            None => return false,
        };

        let mut b_poly = SvgPolygonInner::from_line(b);
        b_poly.correct_winding_order();

        for tri in a_poly.earcut_triangles_iter() {
            let cen = tri.centroid();
            let cen = SvgPoint {
                x: cen.x(),
                y: cen.y(),
            };
            if !point_is_in_polygon(&cen, &b_poly) {
                return true;
            }
        }

        false
    }

    pub fn is_center_inside(a: &SvgLine, b: &SvgLine) -> bool {
        let tr = translate_to_geo_poly_special_shared(&[&SvgPolygonInner::from_line(a)]);
        let a_poly = match tr.0.get(0) {
            Some(s) => s,
            None => return false,
        };

        let mut b_poly = SvgPolygonInner::from_line(b);
        b_poly.correct_winding_order();

        for tri in a_poly.earcut_triangles_iter() {
            let cen = tri.centroid();
            let cen = SvgPoint {
                x: cen.x(),
                y: cen.y(),
            };
            return point_is_in_polygon(&cen, &b_poly);
        }

        return false;
    }

    fn any_points_equal(a: &SvgLine, b: &SvgLine) -> bool {
        let mut a_points = a.points.clone();
        a_points.dedup_by(|a, b| a.equals(b));

        let mut b_points = b.points.clone();
        b_points.dedup_by(|a, b| a.equals(b));

        a_points
            .iter()
            .any(|a| b_points.iter().any(|p| p.dist(a) < 0.005))
    }

    pub fn translate_y(&self, newy: f64) -> Self {
        Self {
            outer_ring: SvgLine {
                points: self.outer_ring
                    .points
                    .iter()
                    .map(|p| SvgPoint {
                        x: p.x,
                        y: p.y + newy,
                    })
                    .collect(),
            },
            inner_rings: self
                .inner_rings
                .iter()
                .map(|s| SvgLine {
                    points: s
                        .points
                        .iter()
                        .map(|p| SvgPoint {
                            x: p.x,
                            y: p.y + newy,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.outer_ring.points.is_empty() && self.inner_rings.is_empty()
    }

    pub fn equals(&self, other: &Self) -> bool {
        self.outer_ring.equals(&other.outer_ring)
        && self.inner_rings.len() == other.inner_rings.len()
        && self
            .inner_rings
            .iter()
            .zip(other.inner_rings.iter())
            .all(|(a, b)| a.equals(b))
    }

    fn round_line(s: &SvgLine) -> SvgLine {
        SvgLine {
            points: s.points.iter().map(SvgPoint::round_to_3dec).collect(),
        }
    }

    pub fn round_to_3dec(&self) -> Self {
        Self {
            outer_ring: Self::round_line(&self.outer_ring),
            inner_rings: self.inner_rings.iter().map(Self::round_line).collect(),
        }
    }

    pub fn get_secondary_label_pos(&self) -> Option<SvgPoint> {
        if self.is_empty() || self.is_zero_area() {
            return None;
        }

        let first_poly = translate_to_geo_poly_special_shared(&[self]).0;
        let first_poly = first_poly.first()?;

        let mut triangles = first_poly.earcut_triangles();

        triangles.sort_by(|a, b| a.unsigned_area().total_cmp(&b.unsigned_area()));

        triangles.reverse();

        triangles.pop();
        let center = triangles
            .iter()
            .next()
            .map(|second_largest| second_largest.centroid())?;
        Some(SvgPoint {
            x: center.x(),
            y: center.y(),
        })
    }

    pub fn get_tertiary_label_pos(&self) -> Option<SvgPoint> {
        if self.is_empty() || self.is_zero_area() {
            return None;
        }

        let first_poly = translate_to_geo_poly_special_shared(&[self]).0;
        let first_poly = first_poly.first()?;

        let mut triangles = first_poly.earcut_triangles();

        if triangles.len() < 3 {
            let center = self.get_rect().get_center();
            return Some(SvgPoint {
                x: center.x,
                y: center.y,
            });
        }

        triangles.sort_by(|a, b| a.unsigned_area().total_cmp(&b.unsigned_area()));

        triangles.reverse();

        triangles.pop();
        triangles.pop();
        let center = triangles
            .iter()
            .next()
            .map(|second_largest| second_largest.centroid())?;
        Some(SvgPoint {
            x: center.x(),
            y: center.y(),
        })
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

        let first_poly = translate_to_geo_poly_special_shared(&[self]).0;
        let first_poly = first_poly.first()?;

        let triangles = first_poly.earcut_triangles();

        let largest_triangle = triangles
            .iter()
            .max_by_key(|t| (t.unsigned_area() * 1000.0) as usize)?;

        let center = largest_triangle.centroid();

        Some(SvgPoint {
            x: center.x(),
            y: center.y(),
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SvgLine {
    pub points: Vec<SvgPoint>,
}

impl SvgLine {
    pub fn to_points_vec(&self) -> Vec<(SvgPoint, SvgPoint)> {
        let mut v = Vec::new();
        for p in self.points.windows(2) {
            match &p {
                &[a, b] => v.push((*a, *b)),
                _ => {}
            }
        }
        v
    }

    pub fn get_hash(&self) -> [u64; 4] {
        use highway::{
            HighwayHash,
            HighwayHasher,
        };
        let rounded = SvgPolygonInner::from_line(self)
            .round_to_3dec()
            .get_all_pointcoords_sorted();
        let bytes = rounded
            .iter()
            .flat_map(|[a, b]| {
                let mut a = a.to_le_bytes().to_vec();
                a.extend(b.to_le_bytes().into_iter());
                a
            })
            .collect::<Vec<_>>();
        let res3: [u64; 4] = HighwayHasher::default().hash256(&bytes);
        res3
    }

    pub fn inverse_point_order(&self) -> SvgLine {
        SvgLine {
            points: {
                let mut newp = self.points.clone();
                newp.reverse();
                newp
            },
        }
    }

    pub fn insert_points_from(&self, other: &SvgPolygonInner, maxdst: f64) -> SvgLine {
        use crate::geograf::l_to_points;
        let mut other_lines = l_to_points(&other.outer_ring);
        other_lines.extend(other.inner_rings.iter().flat_map(|ol| l_to_points(ol)));

        let mut newpoints = self
            .points
            .iter()
            .flat_map(|p| {
                let mut nearest_other_line = other_lines
                    .iter()
                    .filter_map(|(start, end)| {
                        let dst = dist_to_segment(*p, *start, *end);
                        if dst.distance < maxdst {
                            Some(dst.nearest_point)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                nearest_other_line.sort_by(|a, b| a.dist(p).total_cmp(&b.dist(p)));

                let mut ret = vec![*p];
                if let Some(first) = nearest_other_line.first() {
                    ret.push(*first);
                }
                ret
            })
            .collect::<Vec<_>>();

        if let Some(last) = self.points.last() {
            newpoints.push(*last);
        }
        
        newpoints.dedup_by(|a, b| a.equals(b));

        SvgLine { points: newpoints }
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
        self.points.len() == other.points.len()
            && self
                .points
                .iter()
                .zip(other.points.iter())
                .all(|(a, b)| a.equals(b))
    }

    pub fn get_rect(&self) -> quadtree_f32::Rect {
        SvgPolygonInner {
            outer_ring: self.clone(),
            inner_rings: Vec::new(),
        }
        .get_rect()
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
        approx_eq!(f64, self.x, other.x, epsilon = 0.001)
            && approx_eq!(f64, self.y, other.y, epsilon = 0.001)
    }

    pub fn equals_approx(&self, other: &Self, epsilon: f64) -> bool {
        approx_eq!(f64, self.x, other.x, epsilon = epsilon)
            && approx_eq!(f64, self.y, other.y, epsilon = epsilon)
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

impl Eq for SvgPoint {}

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
pub fn parse_nas_xml(xml: Vec<XmlNode>, whitelist: &BTreeSet<String>) -> Result<NasXMLFile, String> {
    // CRS parsen

    let mut crs: Option<String> = None;
    let crs_nodes = get_all_nodes_in_subtree(&xml, "AA_Koordinatenreferenzsystemangaben");
    for c in crs_nodes {
        match get_all_nodes_in_subtree(&c.children, "standard").first() {
            Some(XmlNode { text: Some(s), .. }) if s == "true" => {}
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
    let objekte_nodes = get_all_nodes_in_subtree(&xml, "member");
    let mut objekte = BTreeMap::new();
    for o in objekte_nodes.iter() {
        let o_node = match o.children.first() {
            Some(s) => s,
            None => continue,
        };
        let _flst_id = match o_node.attributes.get("id") {
            Some(s) => s.clone(),
            None => continue,
        };
        if !whitelist.contains(o_node.node_type.as_str()) {
            continue;
        }
        let key = o_node.node_type.clone();
        let poly = xml_select_svg_polygon(&o_node.children);
        if poly.is_empty() {
            continue;
        };

        let mut attributes = o_node
            .children
            .iter()
            .filter_map(|cn| match &cn.text {
                Some(s) => Some((cn.node_type.clone(), s.clone())),
                None => None,
            })
            .collect::<BTreeMap<_, _>>();
        attributes.extend(o_node.attributes.clone().into_iter());
        attributes.insert("AX_Ebene".to_string(), key.clone());

        for s in poly {
            let tp = TaggedPolygon { poly: s, attributes: attributes.clone() };
            objekte.entry(key.clone()).or_insert_with(|| Vec::new()).push(tp);
        }
    }

    Ok(NasXMLFile {
        crs: crs,
        ebenen: objekte,
    })
}

enum LineType {
    LineStringSegment { points: Vec<SvgPoint> },
    Arc { points: Vec<SvgPoint> },
}

impl LineType {
    pub fn get_points(&self) -> Vec<SvgPoint> {
        match self {
            LineType::LineStringSegment { points } => points.clone(),
            LineType::Arc { points } => {
                // TODO: incorrect, but probably ok for now
                points.clone()
            },
        }
    }
}

fn get_children_points(s: &XmlNode) -> Vec<SvgPoint> {
    s.children
    .iter()
    .filter_map(|s| s.text.clone())
    .flat_map(|text| {

        let pts = text
            .split_whitespace()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect::<Vec<_>>();

        pts.chunks(2)
            .filter_map(|f| match f {
                [east, false_north] => Some(SvgPoint {
                    x: *east,
                    y: *false_north,
                }),
                _ => None,
            })
            .collect::<Vec<_>>()

    }).collect()
}

fn xml_select_svg_polygon(xml: &Vec<XmlNode>) -> Vec<SvgPolygonInner> {
    let patches = get_all_nodes_in_subtree(&xml, "PolygonPatch");
    if patches.is_empty() {
        return Vec::new();
    }

    let mut outer_rings = Vec::new();
    let mut inner_rings = Vec::new();
    let children = patches
        .iter()
        .flat_map(|s| s.children.clone())
        .collect::<Vec<_>>();

    for e_i in children.iter() {
        let external = match e_i.node_type.as_str() {
            "exterior" => true,
            "interior" => false,
            _ => continue,
        };

        let linestrings = get_all_nodes_in_subtree(&e_i.children, "segments");

        let linestring_points = linestrings
            .iter()
            .flat_map(|s| {
                s.children.iter().filter_map(|s| {
                    match s.node_type.as_str() {
                        "LineStringSegment" => Some(LineType::LineStringSegment { points: get_children_points(s) }),
                        "Arc" => Some(LineType::Arc { points: get_children_points(s) }),
                        _ => None,
                    }
                })
            })
            .collect::<Vec<_>>();

        let mut line_points = linestring_points
            .into_iter()
            .flat_map(|f| f.get_points())
            .collect::<Vec<_>>();

        line_points.dedup();
        
        if line_points.len() < 3 {
            return Vec::new();
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
        return Vec::new();
    }

    recombine_polys(&outer_rings, &inner_rings)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberObject {
    // AX_...
    pub member_type: String,
    pub beginnt: DateTime<chrono::FixedOffset>,
    pub dient_zur_darstellung_von: Option<String>,
    pub ist_bestandteil_von: Option<String>,
    pub hat: Option<String>,
    pub ist_teil_von: Option<String>,
    pub extra_attribute: BTreeMap<String, String>,
    pub poly: Vec<SvgPolygonInner>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct NasXmlObjects {
    pub objects: BTreeMap<String, MemberObject>,
}

pub fn parse_nas_xml_objects(xml: &Vec<XmlNode>) -> NasXmlObjects {
    let mut map = BTreeMap::new();

    let objekte_nodes = get_all_nodes_in_subtree(&xml, "member");

    for o in objekte_nodes.iter() {
        let o_node = match o.children.first() {
            Some(s) => s,
            None => continue,
        };

        let id = match o_node.attributes.get("id").map(|s| s.clone()) {
            Some(s) => s,
            None => continue,
        };

        let beginnt = match o_node
            .select_subitems(&["lebenszeitintervall", "AA_Lebenszeitintervall", "beginnt"])
            .first()
            .and_then(|s| DateTime::parse_from_rfc3339(&s.text.as_ref()?).ok())
        {
            Some(s) => s,
            None => continue,
        };

        let member_type = o_node.node_type.clone();

        let dient_zur_darstellung_von = o_node
            .select_subitems(&["dientZurDarstellungVon"])
            .first()
            .and_then(|s| s.attributes.get("href").cloned());

        let ist_bestandteil_von = o_node
            .select_subitems(&["istBestandteilVon"])
            .first()
            .and_then(|s| s.attributes.get("href").cloned());

        let ist_teil_von = o_node
            .select_subitems(&["istTeilVon"])
            .first()
            .and_then(|s| s.attributes.get("href").cloned());

        let hat = o_node
            .select_subitems(&["hat"])
            .first()
            .and_then(|s| s.attributes.get("href").cloned());

        let poly = xml_select_svg_polygon(
            &o_node
                .select_subitems(&["position"])
                .into_iter()
                .cloned()
                .collect(),
        );

        let extra_attribute = o_node
            .children
            .iter()
            .filter_map(|s| Some((s.node_type.clone(), s.text.clone()?)))
            .collect();

        map.insert(
            id,
            MemberObject {
                member_type,
                beginnt,
                dient_zur_darstellung_von,
                ist_bestandteil_von,
                hat,
                ist_teil_von,
                extra_attribute,
                poly,
            },
        );
    }

    NasXmlObjects { objects: map }
}

fn get_proj_string(input: &str) -> Option<String> {
    let mut known_strings = BTreeMap::new();
    for i in 0..60 {
        known_strings.insert(
            format!("ETRS89_UTM{i}"),
            format!("+proj=utm +ellps=GRS80 +units=m +no_defs +zone={i}"),
        );
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

pub fn reproject_line(
    line: &SvgLine,
    source: &Proj,
    target: &Proj,
    use_radians: UseRadians,
) -> SvgLine {
    SvgLine {
        points: line
            .points
            .iter()
            .filter_map(|p| reproject_point(p, source, target, use_radians))
            .collect(),
    }
}

pub fn reproject_point(
    p: &SvgPoint,
    source: &Proj,
    target: &Proj,
    use_radians: UseRadians,
) -> Option<SvgPoint> {
    let mut point3d = if use_radians.for_source() {
        (p.x.to_radians(), p.y.to_radians(), 0.0_f64)
    } else {
        (p.x, p.y, 0.0_f64)
    };
    proj4rs::transform::transform(source, target, &mut point3d).ok()?;
    Some(SvgPoint {
        x: if use_radians.for_target() {
            point3d.0
        } else {
            point3d.0.to_degrees()
        },
        y: if use_radians.for_target() {
            point3d.1
        } else {
            point3d.1.to_degrees()
        },
    })
}

pub fn reproject_poly(
    poly: &SvgPolygonInner,
    source_proj: &proj4rs::Proj,
    target_proj: &proj4rs::Proj,
    use_radians: UseRadians,
    round_3dec: bool,
) -> SvgPolygonInner {
    let s = SvgPolygonInner {
        outer_ring: reproject_line(&poly.outer_ring, &source_proj, &target_proj, use_radians),
        inner_rings: poly
            .inner_rings
            .iter()
            .map(|l| reproject_line(l, &source_proj, &target_proj, use_radians))
            .collect(),
    };

    if round_3dec {
        s.round_to_3dec()
    } else {
        s
    }
}

pub fn transform_nas_xml_to_lat_lon(
    input: &NasXMLFile,
    _log: &mut Vec<String>,
) -> Result<NasXMLFile, String> {
    let source_proj = Proj::from_proj_string(&input.crs)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", input.crs))?;

    let latlon_proj = Proj::from_proj_string(LATLON_STRING)
        .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    let objekte = input
        .ebenen
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.iter()
                    .map(|v| TaggedPolygon {
                        attributes: v.attributes.clone(),
                        poly: reproject_poly(
                            &v.poly, 
                            &source_proj, 
                            &latlon_proj, 
                            UseRadians::None, 
                            false
                        ),
                    })
                    .collect(),
            )
        })
        .collect();

    Ok(NasXMLFile {
        ebenen: objekte,
        crs: LATLON_STRING.to_string(),
    })
}

pub fn transform_split_nas_xml_to_lat_lon(
    input: &SplitNasXml,
    _log: &mut Vec<String>,
) -> Result<SplitNasXml, String> {
    let source_proj = Proj::from_proj_string(&input.crs)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", input.crs))?;
    let latlon_proj_string = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";
    let latlon_proj = Proj::from_proj_string(latlon_proj_string)
        .map_err(|e| format!("latlon_proj_string: {e}: {latlon_proj_string:?}"))?;

    let flurstuecke_nutzungen = input
        .flurstuecke_nutzungen
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.iter()
                    .map(|v| TaggedPolygon {
                        attributes: v.attributes.clone(),
                        poly: SvgPolygonInner {
                            outer_ring: reproject_line(&v.poly.outer_ring, &source_proj, &latlon_proj, UseRadians::None),
                            inner_rings: v
                                .poly
                                .inner_rings
                                .iter()
                                .map(|l| {
                                    reproject_line(l, &source_proj, &latlon_proj, UseRadians::None)
                                })
                                .collect(),
                        },
                    })
                    .collect(),
            )
        })
        .collect();

    Ok(SplitNasXml {
        flurstuecke_nutzungen: flurstuecke_nutzungen,
        crs: latlon_proj_string.to_string(),
    })
}

pub fn fixup_flst_groesse(unprojected: &SplitNasXml, projected: &mut SplitNasXml) {
    for (key, up_polys) in unprojected.flurstuecke_nutzungen.iter() {
        let p_polys = match projected.flurstuecke_nutzungen.get_mut(key) {
            Some(s) => s,
            None => continue,
        };
        for (up, p) in up_polys.iter().zip(p_polys.iter_mut()) {
            let up_size = up.get_groesse();
            p.attributes.insert(
                "BerechneteGroesseM2".to_string(),
                up_size.round().to_string(),
            );
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

pub fn default_etrs33() -> String {
    "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33".to_string()
}

impl SplitNasXml {
    pub fn only_retain_gemarkung(&self, target_gemarkung: usize) -> Self {
        Self {
            crs: self.crs.clone(),
            flurstuecke_nutzungen: self
                .flurstuecke_nutzungen
                .iter()
                .filter_map(|(id, polys)| {
                    let oid = FlstIdParsed::from_str(&id).parse_num()?;
                    if oid.gemarkung == target_gemarkung {
                        Some((id.clone(), polys.clone()))
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }

    pub fn as_splitflaechen(&self) -> Vec<AenderungenIntersection> {
        self.flurstuecke_nutzungen
            .iter()
            .flat_map(|(_flst_id, nutzungen)| {
                nutzungen.iter().filter_map(|tp| {
                    let kuerzel = tp.get_auto_kuerzel()?;
                    Some(AenderungenIntersection {
                        alt: kuerzel.clone(),
                        neu: kuerzel,
                        flst_id: tp.get_flurstueck_id()?,
                        poly_cut: tp.poly.clone(),
                        flst_id_part: tp.get_flst_part_id()?,
                    })
                })
            })
            .collect::<Vec<_>>()
    }

    pub fn migrate_future(&self, spliflaechen: &[AenderungenIntersection]) -> Self {
        Self {
            crs: self.crs.clone(),
            flurstuecke_nutzungen: self
                .flurstuecke_nutzungen
                .iter()
                .map(|(k, v)| {
                    let flst_parts_neu = spliflaechen
                        .iter()
                        .filter_map(|ai| {
                            if ai.flst_id == *k {
                                Some(TaggedPolygon {
                                    attributes: TaggedPolygon::get_auto_attributes_for_kuerzel(
                                        &ai.neu,
                                        &[
                                            ("id", &uuid()),
                                            ("AX_Flurstueck", k),
                                            ("AX_IntersectionId", "0"),
                                        ],
                                    ),
                                    poly: ai.poly_cut.clone(),
                                })
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    if flst_parts_neu.is_empty() {
                        (k.clone(), v.clone())
                    } else {
                        (k.clone(), flst_parts_neu)
                    }
                })
                .collect(),
        }
    }

    pub fn get_linien_quadtree(&self) -> LinienQuadTree {
        let mut alle_linie_split_flurstuecke = self
            .flurstuecke_nutzungen
            .iter()
            .flat_map(|(_, s)| {
                s.iter().flat_map(|q| {
                    let mut lines = crate::geograf::l_to_points(&q.poly.outer_ring);
                    lines.extend(
                        q.poly
                            .inner_rings
                            .iter()
                            .flat_map(crate::geograf::l_to_points),
                    );
                    lines
                })
            })
            .collect::<Vec<_>>();
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
            Some(s) => self
                .flurstuecke_nutzungen
                .get(ax_flurstueck)?
                .iter()
                .find(|p| {
                    p.get_ebene().as_deref() == Some(ax_ebene)
                        && p.attributes.get("AX_IntersectionId").map(|s| s.as_str()) == Some(s)
                        && p.attributes.get("id").map(|s| s.as_str()) == Some(cut_obj_id)
                }),
            None => self
                .flurstuecke_nutzungen
                .get(ax_flurstueck)?
                .iter()
                .find(|p| {
                    p.get_ebene().as_deref() == Some(ax_ebene)
                        && p.attributes.get("id").map(|s| s.as_str()) == Some(cut_obj_id)
                }),
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
            ebenen: aenderungen
                .na_polygone_neu
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        vec![TaggedPolygon {
                            attributes: {
                                let mut q = vec![("aenderungID".to_string(), k.to_string())];
                                if let Some(n) = v.nutzung.clone() {
                                    q.push(("nutzung".to_string(), n.to_string()));
                                }
                                q.into_iter().collect()
                            },
                            poly: v.poly.get_inner(),
                        }],
                    )
                })
                .collect::<BTreeMap<_, _>>(),
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
        self.qt
            .get_ids_that_overlap(rect)
            .into_iter()
            .filter_map(|itemid| {
                let (flst_id, i) = self.ebenen_map.get(&itemid)?;
                self.original.ebenen.get(flst_id)?.get(*i).cloned()
            })
            .collect()
    }

    pub fn get_overlapping_ebenen(
        &self,
        poly: &SvgPolygonInner,
        alle_ebenen: &BTreeMap<String, String>,
    ) -> Vec<(String, TaggedPolygon)> {
        let rect = poly.get_rect();
        let alle_ebenen = alle_ebenen.values().collect::<BTreeSet<_>>();

        self.qt
            .get_ids_that_overlap(&rect)
            .into_iter()
            .filter_map(|itemid| {
                let (flst_id, i) = self.ebenen_map.get(&itemid)?;
                let tp = self.original.ebenen.get(flst_id)?.get(*i)?;
                let ebene = tp.get_ebene()?;
                if alle_ebenen.contains(&ebene) && poly.overlaps(&tp.poly) {
                    Some((ebene, tp.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    // return = empty if points not on any flst line
    pub fn get_line_between_points(
        &self,
        start: &SvgPoint,
        end: &SvgPoint,
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
            let v = p.get_line_between_points(start, end, maxdst_line2, maxdev_followline);
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
                    b.push(poly.poly.outer_ring.clone());
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
        self.qt
            .get_ids_that_overlap(rect)
            .into_iter()
            .filter_map(|itemid| {
                let (flst_id, i) = self.flst_nutzungen_map.get(&itemid)?;
                Some((
                    flst_id.clone(),
                    self.original
                        .flurstuecke_nutzungen
                        .get(flst_id)?
                        .get(*i)
                        .cloned()?,
                ))
            })
            .collect()
    }
}

pub fn split_xml_flurstuecke_inner(
    input: &NasXMLFile,
    _log: &mut Vec<String>,
) -> Result<SplitNasXml, String> {
    log_status("split xml flurstuecke...");
    let mut input = input.clone();
    let mut default = SplitNasXml {
        crs: input.crs.clone(),
        flurstuecke_nutzungen: BTreeMap::new(),
    };
    let ax_flurstuecke = input.ebenen.remove("AX_Flurstueck").unwrap_or_default();
    log_status(&format!("splitting {} flurstuecke", ax_flurstuecke.len()));
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

    log_status(&format!("splitting {itemid} items"));

    let nutzungs_qt = QuadTree::new(
        btree_id_to_poly
            .iter()
            .map(|(k, v)| (ItemId(*k), Item::Rect(v.get_rect()))),
    );

    log_status(&format!("nutzungsqt ok!"));

    let flurstuecke_nutzungen = ax_flurstuecke
        .iter()
        .filter_map(|flst| {
            let id = flst
                .attributes
                .get("flurstueckskennzeichen")?
                .replace("_", "");
            let id = FlstIdParsed::from_str(&id).parse_num()?.format_start_str();

            let bounds = flst.get_rect();
            let ids = nutzungs_qt.get_ids_that_overlap(&bounds);
            let polys = ids
                .iter()
                .filter_map(|i| btree_id_to_poly.get(&i.0))
                .collect::<Vec<_>>();

            let flst_area = flst.poly.area_m2();

            let mut polys = polys
                .iter()
                .flat_map(|p| {
                    let intersection_mp = crate::ops::intersect_polys(&flst.poly, &p.poly);
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
                            let kuerzel = tp.get_auto_kuerzel()?;
                            let nak = TaggedPolygon::get_nutzungsartenkennung(&kuerzel)?;
                            Some((tp, nak))
                        })
                })
                .collect::<Vec<_>>();
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
            log_status(&format!("intersecting {id} done (sum = {sum_poly_areas} m2)"));

            if final_polys.is_empty() {
                None
            } else {
                Some((id.clone(), final_polys))
            }
        })
        .collect();

    log_status(&format!("split ok!"));
    
    Ok(SplitNasXml {
        crs: input.crs.clone(),
        flurstuecke_nutzungen,
    })
}

pub fn cleanup_poly(s: &SvgPolygonInner) -> Vec<SvgPolygonInner> {

    let outer_rings = vec![s.outer_ring.clone()]
        .iter()
        .filter_map(|l| {
            if SvgPolygonInner::from_line(l).is_zero_area() {
                None
            } else {
                Some(l)
            }
        })
        .filter_map(|r| {
            let mut s = SvgPolygonInner::from_line(r);
            s.correct_winding_order();
            if s.is_zero_area() {
                None
            } else {
                Some(s.outer_ring.clone())
            }
        })
        .flat_map(|r| clean_ring_2(&r))
        .collect::<Vec<_>>();

    let inner_rings = s
        .inner_rings
        .iter()
        .filter_map(|l| {
            if SvgPolygonInner::from_line(l).is_zero_area() {
                None
            } else {
                Some(l)
            }
        })
        .filter_map(|r| {
            let mut s = SvgPolygonInner::from_line(r);
            s.correct_winding_order();
            if s.is_zero_area() {
                None
            } else {
                Some(s.outer_ring.clone())
            }
        })
        .flat_map(|r| clean_ring_2(&r))
        .map(|l| l.reverse())
        .collect::<Vec<_>>();

    recombine_polys(&outer_rings, &inner_rings)
    .into_iter()
    .filter_map(|q| if q.is_zero_area() { None } else { Some(q) })
    .collect()
}

pub fn recombine_polys(outer_rings: &[SvgLine], inner_rings: &[SvgLine]) -> Vec<SvgPolygonInner> {  
    outer_rings
    .iter()
    .map(|p| {
        let tr_p = translate_to_geo_poly_special_shared(&[&SvgPolygonInner::from_line(p)]);
        SvgPolygonInner {
            outer_ring: p.clone(),
            inner_rings: inner_rings
            .iter()
            .filter_map(|q| {
                if translate_to_geo_poly_special_shared(&[&SvgPolygonInner::from_line(q)]).is_within(&tr_p) {
                    Some(q.clone())
                } else {
                    None
                }
            })
            .collect(),
        }
    })
    .collect()
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
    let mut lines = points
        .windows(2)
        .map(|a| match &a {
            &[a, b] => vec![*a, *b],
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();

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

    let mut points = lines
        .into_iter()
        .flat_map(|v| v.into_iter())
        .collect::<Vec<_>>();
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
        let poly = SvgPolygonInner::from_line(&line);
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
    v.push(SvgLine { points: newpoints });
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
            &[a, b] => {
                if ray_intersects_segment(p, (a, b)) {
                    count += 1;
                }
            }
            _ => {}
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
        (b.y - a.y) / (b.x - a.x)
    } else {
        std::f64::INFINITY
    };

    let m_blue = if a.x != p.x {
        (p.y - a.y) / (p.x - a.x)
    } else {
        std::f64::INFINITY
    };

    m_blue >= m_red
}

pub fn point_is_in_polygon(p: &SvgPoint, poly: &SvgPolygonInner) -> bool {
    let mut c_in_outer = false;
    if point_in_line(p, &poly.outer_ring) {
        c_in_outer = true;
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
        (self.is_1.points_inside_other_poly == 0 && self.is_2.points_inside_other_poly == 0)
            && (self.is_1.points_touching_lines != 0 || self.is_2.points_touching_lines != 0)
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

    pub fn is_equal(&self) -> bool {
        self.is_1.all_points_are_on_line || self.is_2.all_points_are_on_line
    }
    
    pub fn overlaps(&self) -> bool {
        self.is_1.overlaps_other_poly() || self.is_2.overlaps_other_poly()
    }

    pub fn a_contained_in_b(&self) -> bool {
        self.is_1.is_contained_in_other_poly()
    }

    pub fn b_contained_in_a(&self) -> bool {
        self.is_2.is_contained_in_other_poly()
    }
}

pub fn relate(a: &SvgPolygonInner, b: &SvgPolygonInner, dst: f64) -> Relate {
    let is_1 = only_touches_internal(a, b, dst);
    let is_2 = only_touches_internal(b, a, dst);
    Relate { is_1, is_2 }
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
pub fn only_touches_internal(
    a: &SvgPolygonInner,
    b: &SvgPolygonInner,
    dst: f64,
) -> SvgPolyInternalResult {
    let points_a = a.outer_ring.points.clone();

    // let b_geo = translate_to_geo_poly(b);

    let mut points_touching_lines = 0;
    let mut points_inside_other_poly = 0;
    let mut all_points_are_on_line = true;
    for start_a in points_a.iter() {
        if point_is_on_any_line(start_a, &b, dst) {
            points_touching_lines += 1;
        } else if point_is_in_polygon(start_a, &b) {
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
        all_points_are_on_line,
    }
}

pub fn point_is_on_any_line(p: &SvgPoint, poly: &SvgPolygonInner, dst: f64) -> bool {
    for q in poly.outer_ring.points.windows(2) {
        match &q {
            &[sa, eb] => {
                if dist_to_segment(*p, *sa, *eb).distance < dst {
                    return true;
                }
            }
            _ => {}
        }
    }

    for line in poly.inner_rings.iter() {
        for q in line.points.windows(2) {
            match &q {
                &[sa, eb] => {
                    if dist_to_segment(*p, *sa, *eb).distance < dst {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }

    false
}

pub fn translate_to_geo_poly_special(a: &[SvgPolygonInner]) -> geo::MultiPolygon<f64> {
    translate_to_geo_poly_special_shared(&a.iter().collect::<Vec<_>>())
}

pub fn translate_to_geo_poly_special_shared(a: &[&SvgPolygonInner]) -> geo::MultiPolygon<f64> {
    geo::MultiPolygon(
        a.iter()
        .filter_map(|s| {
            let outer = translate_geoline(&s.outer_ring);
            let inner = s
            .inner_rings
            .iter()
            .map(translate_geoline)
            .collect::<Vec<_>>();
            Some(geo::Polygon::new(outer, inner))
        }).collect()
    )
}

pub fn translate_geoline(a: &SvgLine) -> geo::LineString<f64> {
    geo::LineString(
        a.points
            .iter()
            .map(|coord| geo::Coord {
                x: coord.x,
                y: coord.y,
            })
            .collect(),
    )
}

pub fn translate_from_geo_poly(a: &geo::MultiPolygon<f64>) -> Vec<SvgPolygonInner> {
    a.0.iter()
        .map(|s| SvgPolygonInner {
            outer_ring: translate_ring(s.exterior()),
            inner_rings: s.interiors().iter().map(translate_ring).collect(),
        })
        .collect()
}

pub fn translate_from_geo_poly_special(a: &geo::MultiPolygon<f64>) -> Vec<SvgPolygonInner> {
    a.0.iter()
        .flat_map(|s| {
            let mut q = vec![translate_ring(s.exterior())];
            q.extend(s.interiors().iter().map(translate_ring));
            q.iter()
                .map(|l| {
                    let mut p = SvgPolygonInner::from_line(l);
                    p.correct_winding_order();
                    p
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn translate_ring(a: &geo::LineString<f64>) -> SvgLine {
    SvgLine {
        points: a
            .coords_iter()
            .into_iter()
            .map(|coord| SvgPoint {
                x: coord.x,
                y: coord.y,
            })
            .collect(),
    }
}
