use std::collections::BTreeMap;
use serde_derive::{Serialize, Deserialize};
use crate::xml::XmlNode;
use crate::xml::get_all_nodes_in_subtree;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NasSVGFile {
    pub objekte: BTreeMap<String, Shape>,
    pub crs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Shape {
    Point {
        location: SvgPoint,
        attributes: BTreeMap<String, String>,
    },
    Line {
        points: Vec<SvgPoint>,
        attributes: BTreeMap<String, String>,
    },
    Polygon {
        rings: Vec<SvgPolygon>,
        attributes: BTreeMap<String, String>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgLine {
    pub points: Vec<SvgPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgPolygon {
    pub outer_rings: Vec<SvgLine>,
    pub inner_rings: Vec<SvgLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvgPoint {
    pub x: f32,
    pub y: f32,
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
    let objekte_nodes = get_all_nodes_in_subtree(&xml, "wfs:member");
    for o in objekte_nodes.iter() {
        let o_node = match o.children.first() {
            Some(s) => s,
            None => continue,
        };
        if !whitelist.contains(o_node.node_type.as_str()) {
            continue;
        }
        let key = o_node.node_type.clone();
    }

    let objekte = BTreeMap::new();

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