use std::collections::BTreeMap;
use polylabel_mini::LineString;
use polylabel_mini::Point;
use polylabel_mini::Polygon;
use quadtree_f32::Item;
use quadtree_f32::QuadTree;
use quadtree_f32::Rect;
use serde_derive::{Serialize, Deserialize};
use crate::csv::CsvDataType;
use crate::csv::Status;
use crate::ui::Aenderungen;
use crate::xlsx::FlstIdParsed;
use crate::xml::XmlNode;
use crate::xml::get_all_nodes_in_subtree;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NasXMLFile {
    pub ebenen: BTreeMap<String, Vec<TaggedPolygon>>,
    pub crs: String,
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
                    max_x: max_x as f32,
                    max_y: max_y as f32,
                    min_x: min_x as f32,
                    min_y: min_y as f32,
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
            let [[min_y, min_x], [max_y, max_x]] = tp.get_fit_bounds();
            let bounds = Rect {
                max_x: max_x as f32,
                max_y: max_y as f32,
                min_x: min_x as f32,
                min_y: min_y as f32,
            };
            Some((item_id, (gebaeude_id, bounds, tp.clone())))
        }).collect::<BTreeMap<_, _>>();

        // Get intersection of all gebaeude
        let buildings_qt = QuadTree::new(ax_gebaeude_map.iter().map(|(k, v)| {
            (k.clone(), Item::Rect(v.1.clone()))
        }));

        /* 
        return serde_json::to_string_pretty(&GebaeudeDebugMap {
            anzahl_flurstuecke: ax_flurstuecke_map.len(),
            anzahl_gebaeude: ax_gebaeude_map.len(),
            aenderungen: aenderungen.clone(),
        }).unwrap_or_default();
        */
        
        // All buildings witin the given Flst
        let gebaeude_avail = ax_flurstuecke_map
        .iter()
        .flat_map(|(flst_id, flst_rect)| {
            buildings_qt.get_ids_that_overlap(&flst_rect).iter().filter_map(|building_itemid| {
                let building = ax_gebaeude_map.get(&building_itemid)?;
                let already_deleted = aenderungen.gebaude_loeschen.contains(&building.0);
                Some((building.0.clone(), GebaeudeInfo {
                    flst_id: flst_id.clone(),
                    deleted: already_deleted,
                    gebaeude_id: building.0.clone(),
                    poly: building.2.clone(),
                }))
            }).collect::<Vec<_>>().into_iter()
        })
        .collect::<BTreeMap<_, _>>();

        serde_json::to_string(&gebaeude_avail).unwrap_or_default()
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

            let coords_outer = o.poly.outer_rings.iter().flat_map(|line| {
                 line.points.iter().map(|p| (p.x, p.y))
            }).collect::<Vec<_>>();

            let polygon = Polygon {
                exterior: LineString {
                    points: coords_outer.iter().map(|(x, y)| Point {
                        x: *x,
                        y: *y,
                    }).collect()
                },
                interiors: o.poly.inner_rings.iter().map(|l| LineString {
                    points: l.points.iter().map(|p| Point {
                        x: p.x,
                        y: p.y,
                    }).collect()
                }).collect()
            };
            let label_pos = polylabel_mini::polylabel(&polygon, 0.01);

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
            } else if let Some(p) = poly.poly.outer_rings.get(0) {
                let poly = convert_poly_to_string(p, &holes);
                Some(format!(
                    "{{ \"type\": \"Feature\", \"properties\": {{ {feature_map} }}, \"geometry\": {{ \"type\": \"Polygon\", \"coordinates\": {poly} }} }}"))
            } else {
                None
            }
        }).collect::<Vec<_>>().join(",");

        format!("{{ \"type\": \"FeatureCollection\", \"features\": [{geom}] }}")
    }
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
    pub fn get_fit_bounds(&self) -> [[f64;2];2] {
        let mut min_x = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.x).unwrap_or(0.0);
        let mut max_x = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.x).unwrap_or(0.0);
        let mut min_y = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.y).unwrap_or(0.0);
        let mut max_y = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.y).unwrap_or(0.0);
        for l in self.poly.outer_rings.iter() {
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

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SvgLine {
    pub points: Vec<SvgPoint>,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SvgPoint {
    pub x: f64,
    pub y: f64,
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
pub fn parse_nas_xml(s: &str, whitelist: &[String]) -> Result<NasXMLFile, String> {
    let s = match crate::xml::parse_xml_string(s) {
        Ok(o) => o,
        Err(e) => { return Err(format!("XML parse error: {e:?}")); },
    };
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

        let tp = TaggedPolygon {
            poly: SvgPolygon {
                outer_rings,
                inner_rings,
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

pub fn transform_nas_xml_to_lat_lon(input: &NasXMLFile) -> Result<NasXMLFile, String> {
    use proj4rs::Proj;

    fn reproject_line(line: &SvgLine, source: &Proj, target: &Proj) -> SvgLine {
        SvgLine {
            points: line.points.iter().filter_map(|p| {
                let mut point3d = (p.x, p.y, 0.0_f64);
                proj4rs::transform::transform(source, target, &mut point3d).ok()?;
                Some(SvgPoint {
                    x: point3d.0.to_degrees(), 
                    y: point3d.1.to_degrees()
                })
            }).collect()
        }
    }

    let source_proj = Proj::from_proj_string(&input.crs).map_err(|e| format!("source_proj_string: {e}: {:?}", input.crs))?;
    let latlon_proj_string = "+proj=longlat +ellps=WGS84 +datum=WGS84 +no_defs";
    let latlon_proj = Proj::from_proj_string(latlon_proj_string).map_err(|e| format!("latlon_proj_string: {e}: {latlon_proj_string:?}"))?;

    let objekte = input.ebenen.iter()
    .map(|(k, v)| {
        (k.clone(), v.iter().map(|v| {
            TaggedPolygon {
                attributes: v.attributes.clone(),
                poly: SvgPolygon {
                    outer_rings: v.poly.outer_rings.iter().map(|l| reproject_line(l, &source_proj, &latlon_proj)).collect(),
                    inner_rings: v.poly.inner_rings.iter().map(|l| reproject_line(l, &source_proj, &latlon_proj)).collect(),
                }
            }
        }).collect())
    }).collect();

    Ok(NasXMLFile {
        ebenen: objekte,
        crs: latlon_proj_string.to_string(),
    })
}
