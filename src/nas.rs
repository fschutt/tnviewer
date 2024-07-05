use std::collections::BTreeMap;
use serde_derive::{Serialize, Deserialize};
use crate::xml::XmlNode;
use crate::xml::get_all_nodes_in_subtree;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NasXMLFile {
    pub ebenen: BTreeMap<String, Vec<TaggedPolygon>>,
    pub crs: String,
}

impl NasXMLFile {
    /// Returns GeoJSON für die Ebene
    pub fn get_geojson_ebene(&self, layer: &str) -> String {

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

        let objekte = match self.ebenen.get(layer) {
            Some(o) => o,
            None => return String::new(),
        };

        let geom = objekte.iter().filter_map(|poly| {
            let holes = if poly.poly.inner_rings.is_empty() {
                poly.poly.inner_rings.iter().map(convert_svgline_to_string).collect::<Vec<_>>().join(",")
            } else {
                String::new()
            };
            if poly.poly.outer_rings.len() > 1 {
                let polygons = poly.poly.outer_rings.iter().map(|p| convert_poly_to_string(&p, &holes)).collect::<Vec<_>>().join(",");
                Some(format!("{{ \"type\": \"MultiPolygon\", \"coordinates\": [{polygons}] }}"))
            } else if let Some(p) = poly.poly.outer_rings.get(0) {
                let poly = convert_poly_to_string(p, &holes);
                Some(format!("{{ \"type\": \"Polygon\", \"coordinates\": {poly} }}"))
            } else {
                None
            }
        }).collect::<Vec<_>>().join(",");
        format!("{{ \"type\": \"GeometryCollection\", \"geometries\": [{geom}] }}")
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaggedPolygon {
    pub poly: SvgPolygon,
    pub attributes: BTreeMap<String, String>,
}

impl TaggedPolygon {
    pub fn get_fit_bounds(&self) -> [[f64;2];2] {
        let mut min_x = 0.0;
        let mut max_x = 0.0;
        let mut min_y = 0.0;
        let mut max_y = 0.0;
        for l in self.poly.outer_rings.iter() {
            for p in l.points.iter() {
                if p.x < min_x {
                    min_x = p.x;
                }
                if p.x > max_x {
                    max_x = p.x;
                }
                if p.y < min_y {
                    min_y = p.y;
                }
                if p.y > max_y {
                    max_y = p.y;
                }
            }
        }
    
        [
            [min_y, min_x],
            [max_y, max_x]
        ]
    }
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgLine {
    pub points: Vec<SvgPoint>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SvgPolygon {
    pub outer_rings: Vec<SvgLine>,
    pub inner_rings: Vec<SvgLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgPoint {
    pub x: f64,
    pub y: f64,
}

/// Parse the XML, returns [AX_Gebauede => (Polygon)]
pub fn parse_nas_xml(s: &str, whitelist: &[&str]) -> Result<NasXMLFile, String> {
    let s = match crate::xml::parse_xml_string(s) {
        Ok(o) => o,
        Err(e) => { return Err(format!("XML parse error: {e:?}")); },
    };
    xml_nodes_to_nas_svg_file(s, whitelist)
}

fn xml_nodes_to_nas_svg_file(xml: Vec<XmlNode>, whitelist: &[&str]) -> Result<NasXMLFile, String> {

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
