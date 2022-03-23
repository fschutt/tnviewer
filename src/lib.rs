use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn xml_to_xlsx(bytes: Vec<u8>) -> Vec<u8> {
    let mut text_decoder = chardetng::EncodingDetector::new();
    let _ = text_decoder.feed(&bytes[..], true);
    let text_decoder = text_decoder.guess(None, true);
    let mut text_decoder = text_decoder.new_decoder();
    let mut decoded = String::with_capacity(bytes.len() * 2);
    let _ = text_decoder.decode_to_string(&bytes[..], &mut decoded, true);
    let datensaetze = parse_xml(&decoded);
    let xlsx = datensaetze_zu_xlsx(&datensaetze);
    return xlsx;
}

#[derive(Debug, Default, Clone)]
struct Datensatz {
    name: String,
    beschreibung: String,
    wert: String,
    enabled: String,
    restriction: Vec<String>,
    class: String,
}

fn parse_xml(xml: &str) -> Vec<Datensatz> {
    
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut datensaetze = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut inside_setting = false;
    let mut inside_restriction = false;
    let mut last_text = None;
    
    // println!("attributes values: {:?}", e.attributes().map(|a| a.unwrap().value).collect::<Vec<_>>())

    let mut last_datensatz = Datensatz::default();
    
    // The `Reader` does not implement `Iterator` because it outputs borrowed data (`Cow`s)
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                match e.name() {
                    b"class" => {
                        if inside_setting { 
                            let d = e.attributes()
                                .with_checks(false)
                                .filter_map(|a| a.ok())
                                .find_map(|a| if a.key == b"default" { Some(a.value) } else { None })
                                .map(|a| a.into_owned())
                                .unwrap_or_default();
                            let d = String::from_utf8(d).unwrap_or_default();
                            last_datensatz.class = d;
                        }
                    },
                    _ => { },
                }
            },
            Ok(Event::Start(ref e)) => {
                match e.name() {
                    b"setting" => { inside_setting = true; },
                    b"restriction" => { if inside_setting { inside_restriction = true; } },
                    b"name" => { if inside_setting { last_text = Some("name"); } },
                    b"value" => { if inside_setting { last_text = Some("value"); } },
                    b"enabled" => { if inside_setting { last_text = Some("enabled"); } },
                    _ => (),
                }
            },
            Ok(Event::End(ref e)) => {
                match e.name() {
                    b"setting" => { 
                        if inside_setting {
                            datensaetze.push(last_datensatz.clone());
                            last_datensatz = Datensatz::default();
                        }
                        inside_setting = false; 
                    },
                    b"restriction" => { if inside_setting { inside_restriction = false; } },
                    b"name" => { if inside_setting { last_text = None; } },
                    b"value" => { if inside_setting { last_text = None; } },
                    b"enabled" => { if inside_setting { last_text = None; } },
                    _ => (),
                }
            },            
            Ok(Event::Comment(e)) => {
                if inside_setting && last_text.is_none() {
                    last_datensatz.beschreibung.push_str(&e.unescape_and_decode(&reader).unwrap_or_default());
                    last_datensatz.beschreibung = last_datensatz.beschreibung.trim().replace("\t", " ");
                }
                // txt.push()
            },
            Ok(Event::Text(e)) => {
                if inside_setting {
                    match last_text {
                        Some("name") => { last_datensatz.name = e.unescape_and_decode(&reader).unwrap_or_default(); },
                        Some("value") => { 
                            if inside_restriction {
                                last_datensatz.restriction.push(e.unescape_and_decode(&reader).unwrap_or_default()); 
                            } else { 
                                last_datensatz.wert = e.unescape_and_decode(&reader).unwrap_or_default(); 
                            } 
                        },
                        Some("enabled") => { last_datensatz.enabled = e.unescape_and_decode(&reader).unwrap_or_default(); },
                        _ => { },
                    }
                }
            },
            Ok(Event::Eof) => break, // exits the loop when reaching end of file
            Err(_) => return datensaetze,
            _ => { },
        }

        buf.clear();
    }
    
    datensaetze
}

fn datensaetze_zu_xlsx(datensaetze: &[Datensatz]) -> Vec<u8> {
    
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
            let datensatz_beschreibung_lines = d.beschreibung.lines().collect::<Vec<_>>();
            let restriction_len = d.restriction.len();
            let datensatz_beschreibung_lines_len = datensatz_beschreibung_lines.len();
            let max = datensatz_beschreibung_lines_len.max(restriction_len);
            
            let row_start = row;
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
