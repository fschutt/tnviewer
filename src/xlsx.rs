
use crate::csv::CsvDataType;

pub fn generate_report(datensaetze: &CsvDataType) -> Vec<u8> {

    use simple_excel_writer::*;
    
    let mut wb = Workbook::create_in_memory();
    let mut sheet = wb.create_sheet("Preferences");

    // ID
    sheet.add_column(Column { width: 30.0 });
    // Status
    sheet.add_column(Column { width: 30.0 });
    // Eigentümer
    sheet.add_column(Column { width: 60.0 });

        let _ = wb.write_sheet(&mut sheet, |sheet_writer| {
            let sw = sheet_writer;
            sw.append_row(row!["ID", "Status", "Notiz", "Eigentümer"])?;
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
                let eig = eigentuemer.join("; ");
                sw.append_row(row![
                    flst_id.to_string(),
                    match status {
                        crate::csv::Status::Bleibt => "bleibt".to_string(),
                        crate::csv::Status::AenderungKeineBenachrichtigung => notiz + " (keine Benachrichtigung)",
                        crate::csv::Status::AenderungMitBenachrichtigung => notiz + " (keine Benachrichtigung)",
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
