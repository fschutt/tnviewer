
use crate::csv::CsvDataType;
use crate::csv::Status;

pub fn get_veraenderte_flst(datensaetze: &CsvDataType) -> Vec<u8> {
    let all = get_alle_flst_internal(datensaetze);
    all.iter()
    .filter_map(|(v, b)| if *b {
        Some(v.clone())
    } else {
        None
    }).collect::<Vec<String>>()
    .join(",")
    .into()
}

pub fn get_alle_flst(datensaetze: &CsvDataType) -> Vec<u8> {
    let all = get_alle_flst_internal(datensaetze);
    all.iter()
    .map(|(v, _)| v.clone())
    .collect::<Vec<_>>()
    .join(",")
    .into()
}


fn get_alle_flst_internal(datensaetze: &CsvDataType) -> Vec<(String, bool)> {
    // 12 1175 003 00038 00000
    let mut target = Vec::new();
    for (ds, v) in datensaetze.iter() {

        let ds_modified = v
        .get(0)
        .map(|s| s.status == Status::AenderungMitBenachrichtigung)
        .unwrap_or(false);

        let mut chars = ds.chars().collect::<Vec<char>>();
        chars.reverse();
        let mut last_10 = chars.iter().take(10).cloned().collect::<Vec<_>>();
        last_10.reverse();
        if last_10.len() != 10 {
            continue;
        }
        let zaehler = &last_10[..5];
        let nenner = &last_10[5..];
        let zaehler = zaehler.into_iter().collect::<String>();
        let nenner = nenner.into_iter().collect::<String>();
        let z = match zaehler.parse::<usize>() {
            Ok(o) => o,
            Err(_) => continue,
        };
        let n = match nenner.parse::<usize>() {
            Ok(o) => o,
            Err(_) => continue,
        };
        target.push((format!("{z}/{n}"), ds_modified));
    }
    target
}

pub fn generate_report(datensaetze: &CsvDataType) -> Vec<u8> {

    use simple_excel_writer::*;
    
    let mut wb = Workbook::create_in_memory();
    let mut sheet = wb.create_sheet("Preferences");

    // ID
    sheet.add_column(Column { width: 30.0 });
    // Nutzung
    sheet.add_column(Column { width: 60.0 });
    // Status
    sheet.add_column(Column { width: 30.0 });
    // Eigentümer
    sheet.add_column(Column { width: 60.0 });

        let _ = wb.write_sheet(&mut sheet, |sheet_writer| {
            let sw = sheet_writer;
            sw.append_row(row!["ID", "Nutzung", "Status", "Eigentümer"])?;
            for (flst_id, ds) in datensaetze.iter() {
                let ds_0 = match ds.get(0) {
                    Some(s) => s,
                    None => continue
                };
                let notiz = ds_0.notiz.clone();
                let status = ds_0.status.clone();
                let mut eigentuemer = ds.iter().map(|s| s.eigentuemer.clone()).collect::<Vec<_>>();
                eigentuemer.sort();
                eigentuemer.dedup();
                let eig: String = eigentuemer.join("; ");
                let nutzung = ds_0.nutzung.clone();
                sw.append_row(row![
                    flst_id.to_string(),
                    nutzung.to_string(),
                    match status {
                        crate::csv::Status::Bleibt => "bleibt".to_string(),
                        crate::csv::Status::AenderungKeineBenachrichtigung => notiz + " (keine Benachrichtigung)",
                        crate::csv::Status::AenderungMitBenachrichtigung => notiz + " (mit Benachrichtigung)",
                    },
                    eig.to_string()
                ])?;
            }

            Ok(())
        });

    match wb.close() {
        Ok(Some(o)) => o,
        _ => Vec::new(),
    }
}
