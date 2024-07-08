use nas::{NasXMLFile, TaggedPolygon};
use wasm_bindgen::prelude::*;
use crate::ui::UiData;
use crate::csv::CsvDataType;

pub mod xml;
pub mod ui;
pub mod nas;
pub mod csv;
pub mod xlsx;

#[wasm_bindgen]
pub fn ui_render_entire_screen(decoded: String) -> String {
    let uidata = UiData::from_string(&decoded);
    crate::ui::render_entire_screen(&uidata)
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
pub fn ui_render_project_content(decoded: String, csv_data: String) -> String {
    let _uidata = UiData::from_string(&decoded);
    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or(CsvDataType::default());
    crate::ui::render_project_content(csv_data)
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
pub fn load_nas_xml(s: String) -> String {
    let xml = match crate::nas::parse_nas_xml(&s, &["AX_Gebaeude", "AX_Landwirtschaft", "AX_Flurstueck", "AX_Strassenverkehr", "AX_Fliessgewaesser", "AX_Gehoelz", "AX_HistorischesFlurstueck", "AX_Wohnbauflaeche", "AX_GemarkungsteilFlur"]) {
        Ok(o) => o,
        Err(e) => return e,
    };
    match crate::nas::transform_nas_xml_to_lat_lon(&xml) {
        Ok(o) => serde_json::to_string(&o).unwrap_or_default(),
        Err(e) => e,
    }
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

pub fn decode(bytes: Vec<u8>) -> String {
    let mut text_decoder = chardetng::EncodingDetector::new();
    let _ = text_decoder.feed(&bytes[..], true);
    let text_decoder = text_decoder.guess(None, true);
    let mut text_decoder = text_decoder.new_decoder();
    let mut decoded = String::with_capacity(bytes.len() * 2);
    let _ = text_decoder.decode_to_string(&bytes[..], &mut decoded, true);
    decoded
}