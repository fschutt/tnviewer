use nas::NasXMLFile;
use wasm_bindgen::prelude::*;
use crate::ui::UiData;

pub mod xml;
pub mod ui;
pub mod nas;

#[wasm_bindgen]
pub fn ui_render_entire_screen(decoded: String) -> String {
    let uidata = UiData::from_string(&decoded);
    crate::ui::render_entire_screen(&uidata)
}

#[wasm_bindgen]
pub fn load_nas_xml(s: String) -> String {
    let xml = match crate::nas::parse_nas_xml(&s, &["AX_Gebaeude", "AX_Landwirtschaft", "AX_Flurstueck"]) {
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

/* 
fn datensaetze_zu_xlsx(datensaetze: &[Datensatz], inline: bool) -> Vec<u8> {
    
    use simple_excel_writer::*;
    
    let mut wb = Workbook::create_in_memory();
    let mut sheet = wb.create_sheet("Preferences");

    // set column width
    sheet.add_column(Column { width: 50.0 });
    sheet.add_column(Column { width: 500.0 });
    sheet.add_column(Column { width: 20.0 });
    sheet.add_column(Column { width: 20.0 });
    sheet.add_column(Column { width: 20.0 });
    sheet.add_column(Column { width: 20.0 });

    let mut row = 2;
    wb.write_sheet(&mut sheet, |sheet_writer| {
        let sw = sheet_writer;
        sw.append_row(row!["Name", "Beschreibung", "Wert", "Aktiviert", "Wertebereich", "Klasse"])?;
        sw.append_row(row!["<name>", "<!-- ... -->", "<value>","<enabled>", "<restriction>", "<class>"])?;
        
        for d in datensaetze {
            if inline {
                sw.append_row(row![
                    d.name.clone(), 
                    d.beschreibung.clone(), 
                    d.wert.clone(), 
                    d.enabled.clone(), 
                    d.restriction.join(" \r\n"), 
                    d.class.clone()
                ])?;
                continue;
            }
            
            let datensatz_beschreibung_lines = d.beschreibung.lines().collect::<Vec<_>>();
            let restriction_len = d.restriction.len();
            let datensatz_beschreibung_lines_len = datensatz_beschreibung_lines.len();
            let max = datensatz_beschreibung_lines_len.max(restriction_len);
            
            row += 1;
            sw.append_row(row![
                d.name.clone(), 
                datensatz_beschreibung_lines.first().cloned().unwrap_or_default(), 
                d.wert.clone(), 
                d.enabled.clone(), 
                d.restriction.first().cloned().unwrap_or_default(), 
                d.class.clone()
            ])?;
            
            if max > 1 {
                for i in 1..max {
                    let beschreibung_line = datensatz_beschreibung_lines.get(i).cloned().unwrap_or_default();
                    let restriction_line = d.restriction.get(i).cloned().unwrap_or_default();
                    
                    row += 1;
                    sw.append_row(row![
                        "", 
                        beschreibung_line, 
                        "", 
                        "", 
                        restriction_line, 
                        ""
                    ])?;
                }    
            }

            row += 1;
            sw.append_blank_rows(1);
        }
        Ok(())
    }).expect("write excel error!");

    let bytes = wb.close().expect("close excel error!").unwrap_or_default();
    bytes
}
*/

pub fn get_string_from_js_bytes(bytes: &[u8]) -> String {
    let mut text_decoder = chardetng::EncodingDetector::new();
    let _ = text_decoder.feed(&bytes[..], true);
    let text_decoder = text_decoder.guess(None, true);
    let mut text_decoder = text_decoder.new_decoder();
    let mut decoded = String::with_capacity(bytes.len() * 2);
    let _ = text_decoder.decode_to_string(&bytes[..], &mut decoded, true);
    decoded
}
