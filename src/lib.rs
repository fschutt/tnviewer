use std::slice::SplitInclusive;

use nas::{NasXMLFile, SplitNasXml, TaggedPolygon};
use ui::Aenderungen;
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
pub fn ui_render_project_content(decoded: String, csv_data: String, split_flurstuecke: String) -> String {
    let uidata = UiData::from_string(&decoded);
    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or(CsvDataType::default());
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    crate::ui::render_project_content(&csv_data, &uidata, &split_fs)
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
    let nas_cut_projected = match crate::nas::transform_split_nas_xml_to_lat_lon(&nas_cut_original, &mut log) {
        Ok(o) => o,
        Err(e) => return e,
    };
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
        Err(e) => return e.to_string(),
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