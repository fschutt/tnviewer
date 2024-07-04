use std::collections::BTreeMap;
use serde_derive::{Serialize, Deserialize};
use crate::xml::XmlNode;
use crate::xml::get_all_nodes_in_subtree;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NasSVGFile {
    pub objekte: BTreeMap<String, TaggedPolygon>,
    pub crs: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaggedPolygon {
    pub poly: SvgPolygon,
    pub attributes: BTreeMap<String, String>,
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
pub fn parse_nas_xml(s: &str, whitelist: &[&str]) -> NasSVGFile {
    let s = match crate::xml::parse_xml_string(s) {
        Ok(o) => o,
        Err(e) => {
            println!("ERROR {e:?}");
            return NasSVGFile::default();
        },
    };

    xml_nodes_to_nas_svg_file(s, whitelist)
}

fn xml_nodes_to_nas_svg_file(xml: Vec<XmlNode>, whitelist: &[&str]) -> NasSVGFile {

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
        None => return NasSVGFile::default(), // no CRS found
    };
    let crs = crs.replace("urn:adv:crs:", "");

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

        objekte.insert(key, tp);
    }

    NasSVGFile {
        crs: crs,
        objekte,
    }
}

#[test]
fn test_parse_nas() {
    let s = parse_nas_xml(include_str!("../test.xml"), &["AX_Gebaeude", "AX_Landwirtschaft"]);
    println!("{s:#?}");
}