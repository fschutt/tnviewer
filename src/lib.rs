use std::{collections::BTreeMap, slice::SplitInclusive};

use nas::{NasXMLFile, SplitNasXml, SvgPolygon, TaggedPolygon};
use ui::{Aenderungen, PolyNeu};
use wasm_bindgen::prelude::*;
use xml::XmlNode;
use crate::ui::UiData;
use crate::csv::CsvDataType;
use serde_derive::{Serialize, Deserialize};

pub mod xml;
pub mod ui;
pub mod nas;
pub mod csv;
pub mod xlsx;
pub mod search;
pub mod pdf;
pub mod uuid_wasm;
pub mod analyze;

#[wasm_bindgen]
pub fn get_new_poly_id() -> String {
    crate::uuid_wasm::uuid()
}

#[wasm_bindgen]
pub fn get_geojson_fuer_neue_polygone(aenderungen: String) -> String {
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct NeuePolygoneGeoJson {
        nutzung_definiert: bool,
        geojson: String,
    }

    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();

    let construct_polys = |(k, v): (&String, &PolyNeu)| {
        TaggedPolygon {
            attributes: vec![("newPolyId".to_string(), k.clone())].into_iter().collect(),
            poly: v.poly.clone(),
        }
    };

    let nutzung_definiert = aenderungen.na_polygone_neu.iter()
    .filter(|(_, poly)| poly.nutzung.is_some())
    .map(construct_polys).collect::<Vec<_>>();

    let nutzung_definiert = NeuePolygoneGeoJson {
        nutzung_definiert: true,
        geojson: crate::nas::tagged_polys_to_featurecollection(&nutzung_definiert),
    };

    let nutzung_nicht_definiert = aenderungen.na_polygone_neu.iter()
    .filter(|(_, poly)| poly.nutzung.is_none())
    .map(construct_polys).collect::<Vec<_>>();

    let nutzung_nicht_definiert = NeuePolygoneGeoJson {
        nutzung_definiert: false,
        geojson: crate::nas::tagged_polys_to_featurecollection(&nutzung_nicht_definiert),
    };

    serde_json::to_string(&[
        nutzung_definiert,
        nutzung_nicht_definiert
    ]).unwrap_or_default()

}

#[wasm_bindgen]
pub fn get_polyline_guides_in_current_bounds(
    split_flurstuecke: String,
    aenderungen: String,
    map_bounds: String,
) -> String {

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
    struct MapBounds {
        _northEast: crate::analyze::LatLng,
        _southWest: crate::analyze::LatLng,
    }
    
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();
    let MapBounds { _northEast, _southWest } = match serde_json::from_str::<MapBounds>(&map_bounds) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let rect = quadtree_f32::Rect {
        min_x: _southWest.lng,
        min_y: _southWest.lat,
        max_x: _northEast.lng,
        max_y: _northEast.lat,
    };
    let mut ringe = split_fs.get_polyline_guides_in_bounds(rect);
    let mut aenderungen_ringe = aenderungen.na_polygone_neu.values().flat_map(|p| {
        let mut v = p.poly.outer_rings.clone();
        v.append(&mut p.poly.inner_rings.clone());
        v.into_iter()
    }).collect::<Vec<_>>();
    ringe.append(&mut aenderungen_ringe);

    let pl = ringe.iter().map(|svg_line| {
        svg_line.points.iter().map(|p| [p.y, p.x]).collect::<Vec<_>>()
    }).collect::<Vec<_>>();

    serde_json::to_string(&pl).unwrap_or_default()
}

#[wasm_bindgen]
pub fn fixup_polyline(
    xml: String,
    split_flurstuecke: String,
    points: String,
) -> String {
    use crate::analyze::LatLng;
    let xml = serde_json::from_str::<NasXMLFile>(&xml).unwrap_or_default();
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    let points = serde_json::from_str::<Vec<LatLng>>(&points).unwrap_or_default();
    let poly = match crate::analyze::fixup_polyline_internal(&points, &split_fs) {
        Some(s) => s,
        None => return format!("failed to create poly from points {points:?}"),
    };
    serde_json::to_string(&crate::ui::PolyNeu {
        poly: poly,
        nutzung: None,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn ui_render_entire_screen(uidata: String, csv: String, aenderungen: String) -> String {
    let uidata = UiData::from_string(&uidata);
    let csv = serde_json::from_str(&csv).unwrap_or_default();
    let aenderungen = serde_json::from_str(&aenderungen).unwrap_or_default();
    crate::ui::render_entire_screen(&uidata, &csv, &aenderungen)
}

#[wasm_bindgen]
pub fn ui_render_ribbon(decoded: String) -> String {
    let uidata = UiData::from_string(&decoded);
    crate::ui::render_ribbon(&uidata)
}

#[wasm_bindgen]
pub fn ui_render_popover_content(decoded: String) -> String {
    let uidata = UiData::from_string(&decoded);
    crate::ui::render_popover_content(&uidata)
}

#[wasm_bindgen]
pub fn stringify_savefile(csv_data: String, aenderungen: String) -> String {

    #[derive(Debug, Deserialize, Serialize)]
    struct SaveFile {
        csv: CsvDataType,
        aenderungen: Aenderungen,
    }

    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or(CsvDataType::default());
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or(Aenderungen::default());
    serde_json::to_string_pretty(&SaveFile {
        csv: csv_data,
        aenderungen,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn ui_render_project_content(uidata: String, csv_data: String, aenderungen: String, split_flurstuecke: Option<String>) -> String {
    let split_flurstuecke = split_flurstuecke.unwrap_or_default();
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();
    let uidata = UiData::from_string(&uidata);
    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or(CsvDataType::default());
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    crate::ui::render_project_content(&csv_data, &aenderungen, &uidata, &split_fs)
}

#[wasm_bindgen]
pub fn ui_render_secondary_content(aenderungen: String) -> String {
    let aenderungen = serde_json::from_str(&aenderungen).unwrap_or_default();
    crate::ui::render_secondary_content(&aenderungen)
}

#[wasm_bindgen]
pub fn get_fit_bounds(s: String) -> String {
    let flst = match serde_json::from_str::<TaggedPolygon>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string()
    };
    let bounds = flst.get_fit_bounds();
    serde_json::to_string(&bounds).unwrap_or_default()
}

#[wasm_bindgen]
pub fn search_for_polyneu(aenderungen: String, poly_id: String) -> String {
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let tp = aenderungen.na_polygone_neu.iter()
    .find_map(|(k, v)| if k.as_str() == poly_id.as_str() {
        Some(TaggedPolygon {
            attributes: BTreeMap::new(),
            poly: v.poly.clone(),
        })
    } else {
        None
    });

    match tp {
        Some(s) => serde_json::to_string(&s.get_fit_bounds()).unwrap_or_default(),
        None => String::new(),
    }
}

#[wasm_bindgen]
pub fn search_for_gebauede(s: String, gebaeude_id: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let ax_gebauede = match xml.ebenen.get("AX_Gebaeude") {
        Some(s) => s,
        None => return "keine Ebene AX_Gebaeude".to_string(),
    };
    let r =  ax_gebauede
    .iter()
    .find(|f| f.attributes.get("id") == Some(&gebaeude_id));
    
    match r {
        Some(s) => serde_json::to_string(&s.get_fit_bounds()).unwrap_or_default(),
        None => String::new(),
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LoadNasReturn {
    pub log: Vec<String>,
    pub xml_parsed: Vec<XmlNode>,
    pub nas_original: NasXMLFile,
    pub nas_cut_original: SplitNasXml,
    pub nas_projected: NasXMLFile,
    pub nas_cut_projected: SplitNasXml,
}

#[wasm_bindgen]
pub fn load_nas_xml(s: String, types: String) -> String {
    let mut t = types.split(",").filter_map(|s| {
        let s = s.trim();
        if s.is_empty() { None } else { Some(s.to_string()) }
    }).collect::<Vec<_>>();
    t.sort();
    t.dedup();
    let mut log = Vec::new();
    log.push(format!("parsing XML: types = {t:?}"));
    let xml_parsed = match crate::xml::parse_xml_string(&s, &mut log) {
        Ok(o) => o,
        Err(e) => return format!("XML parse error: {e:?}"),
    };
    let nas_original = match crate::nas::parse_nas_xml(xml_parsed.clone(), &t, &mut log) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let nas_cut_original = match crate::nas::split_xml_flurstuecke_inner(&nas_original, &mut log) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let nas_projected = match crate::nas::transform_nas_xml_to_lat_lon(&nas_original, &mut log) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let mut nas_cut_projected = match crate::nas::transform_split_nas_xml_to_lat_lon(&nas_cut_original, &mut log) {
        Ok(o) => o,
        Err(e) => return e,
    };
    crate::nas::fixup_flst_groesse(&nas_cut_original, &mut nas_cut_projected);
    serde_json::to_string(&LoadNasReturn {
        log,
        xml_parsed,
        nas_original,
        nas_cut_original,
        nas_projected,
        nas_cut_projected,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn get_geojson_fuer_ebene(json: String, layer: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(e) => return "ERROR HERE".to_string() + &e.to_string(),
    };
    xml.get_geojson_ebene(&layer)
}

#[wasm_bindgen]
pub fn get_gebaeude_geojson_fuer_aktive_flst(json: String, csv: String, aenderungen: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let csv = match serde_json::from_str::<CsvDataType>(&csv) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    xml.get_gebaeude(&csv, &aenderungen)
}

#[wasm_bindgen]
pub fn get_labels_fuer_ebene(json: String, layer: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let labels = xml.get_geojson_labels(&layer);
    serde_json::to_string(&labels).unwrap_or_default()
}


#[wasm_bindgen]
pub fn parse_csv_dataset_to_json(
    csv: Vec<u8>, 
    id_col: String, 
    nutzung_col: String, 
    eigentuemer_col: String, 
    delimiter: String,
    ignore_firstline: String
) -> String {
    let csv = decode(csv);
    let csv_daten = match crate::csv::parse_csv(
        &csv, 
        &id_col, 
        &nutzung_col, 
        &eigentuemer_col, 
        &delimiter,
        ignore_firstline == "true"
    ) {
        Ok(o) => o,
        Err(e) => return e,
    };
    serde_json::to_string(&csv_daten).unwrap_or_default()
}

#[wasm_bindgen]
pub fn export_veraenderte_flst(s: String) -> Vec<u8> {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    crate::xlsx::get_veraenderte_flst(&data)
}

#[wasm_bindgen]
pub fn export_alle_flst(s: String) -> Vec<u8> {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    crate::xlsx::get_alle_flst(&data)
}

#[wasm_bindgen]
pub fn export_xlsx(s: String) -> Vec<u8> {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    crate::xlsx::generate_report(&data)
}

#[wasm_bindgen]
pub fn export_flst_id_nach_eigentuemer(s: String) -> Vec<u8> {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    crate::xlsx::flst_id_nach_eigentuemer(&data)
}

#[wasm_bindgen]
pub fn export_pdf(csv: String, json: String) -> Vec<u8> {
    let csv = match serde_json::from_str::<CsvDataType>(&csv) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    crate::pdf::generate_pdf(&csv, &xml)
}

pub fn decode(bytes: Vec<u8>) -> String {
    let mut text_decoder = chardetng::EncodingDetector::new();
    let _ = text_decoder.feed(&bytes[..], true);
    let text_decoder = text_decoder.guess(None, true);
    let mut text_decoder = text_decoder.new_decoder();
    let mut decoded = String::with_capacity(bytes.len() * 2);
    let _ = text_decoder.decode_to_string(&bytes[..], &mut decoded, true);
    decoded
}