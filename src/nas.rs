use std::{collections::BTreeMap, f32::consts::E};
use serde_derive::{Serialize, Deserialize};

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
pub fn parse_nas_xml(s: &str) -> NasSVGFile {
    let s = match crate::xml::parse_xml_string(s) {
        Ok(o) => o,
        Err(e) => {
            println!("ERROR {e:?}");
            return NasSVGFile::default();
        },
    };

    // println!("{s:#?}");
    NasSVGFile::default()
}

#[test]
fn test_parse_nas() {
    let s = parse_nas_xml(include_str!("../test.xml"));
    // println!("{s:#?}");
}