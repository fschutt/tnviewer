
use std::collections::BTreeMap;
use std::collections::BTreeSet;

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
        let mut last_11 = chars.iter().take(11).cloned().collect::<Vec<_>>();
        last_11.reverse();
        if last_11.len() != 11 {
            continue;
        }
        let zaehler = &last_11[..5];
        let nenner = &last_11[5..9];
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
    let mut sheet = wb.create_sheet("Flurstuecke");

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
                    FlstIdParsed::from_str(&flst_id).to_nice_string(),
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

pub fn flst_id_nach_eigentuemer(datensaetze: &CsvDataType) -> Vec<u8> {

    use simple_excel_writer::*;
    
    let mut wb = Workbook::create_in_memory();
    let mut sheet = wb.create_sheet("FlstIdNachEigentuemer");

    // Eigentuemer
    sheet.add_column(Column { width: 30.0 });
    // Flurstuecke
    sheet.add_column(Column { width: 60.0 });

    let mut eigentuemer = BTreeMap::new();
    for (k, v) in datensaetze.iter() {
        for d in v.iter() {
            if d.status != Status::AenderungMitBenachrichtigung {
                continue;
            }
            let flst = match FlstIdParsed::from_str(k).parse_num() {
                Some(s) => s,
                None => continue,
            };
            eigentuemer
            .entry(d.eigentuemer.trim().to_string())
            .or_insert_with(|| Vec::new())
            .push(flst);
        }
    }

    let _ = wb.write_sheet(&mut sheet, |sheet_writer| {
        let sw = sheet_writer;
        sw.append_row(row!["Eigentümer", "Flurstücke"])?;

        for (k, v) in eigentuemer.iter() {
            let mut v = v.clone();
            v.sort_by(|a, b| a.flst_zaehler.cmp(&b.flst_zaehler));
            v.dedup();
            sw.append_row(row![
                k.to_string(),
                v.iter().map(|q| q.format_str()).collect::<Vec<_>>().join(", ")
            ])?;
        }

        Ok(())
    });

    match wb.close() {
        Ok(Some(o)) => o,
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct FlstIdParsed {
    pub land: String, 
    pub gemarkung: String,
    pub flur: String,
    pub flst_zaehler: String,
    pub flst_nenner: String,
    pub padding: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct FlstIdParsedNumber {
    pub land: usize, 
    pub gemarkung: usize,
    pub flur: usize,
    pub flst_zaehler: usize,
    pub flst_nenner: Option<usize>,
}

impl FlstIdParsedNumber {

    pub fn format_start_str(&self) -> String {
        let FlstIdParsedNumber { land, gemarkung, flur, flst_zaehler, flst_nenner } = self;
        // 12118000300001 0000 00
        let n = if let Some(n) = flst_nenner { format!("{n:05}") } else { String::new() };
        format!("{land:02}{gemarkung:04}{flur:03}{flst_zaehler:05}{n}")
    }

    pub fn get_flur(&self) -> String {
        self.flur.to_string()
    }
    
    pub fn format_str(&self) -> String {
       match self.flst_nenner {
        Some(0) | None => self.flst_zaehler.to_string(),
        Some(s) => format!("{}/{}", self.flst_zaehler, s),
       }
    }
}

impl FlstIdParsed {

    pub fn from_str(id: &str) -> FlstIdParsed {

        // 12 1180 003 00001 0000 00
        // 12-1180-003-00261/0000

        let chars = id.chars().collect::<Vec<_>>();
        let land = chars.iter().skip(0).take(2).collect::<String>();
        let gemarkung = chars.iter().skip(2).take(4).collect::<String>();
        let flur = chars.iter().skip(6).take(3).collect::<String>();
        let flst_zaehler = chars.iter().skip(9).take(5).collect::<String>();
        let flst_nenner = chars.iter().skip(14).take(4).collect::<String>();
        let padding = chars.iter().skip(18).take(2).collect::<String>();

        FlstIdParsed {
            land, 
            gemarkung,
            flur,
            flst_zaehler,
            flst_nenner,
            padding,
        }
    }

    pub fn to_nice_string(&self) -> String {
        let FlstIdParsed { 
            land, 
            gemarkung, 
            flur, 
            flst_zaehler, 
            flst_nenner, 
            .. 
        } = self;

        format!("{land}-{gemarkung}-{flur}-{flst_zaehler}/{flst_nenner}")
    }

    pub fn parse_num(&self) -> Option<FlstIdParsedNumber> {
        Some(FlstIdParsedNumber {
            land: self.land.trim().parse::<usize>().ok()?, 
            gemarkung: self.gemarkung.trim().parse::<usize>().ok()?, 
            flur: self.flur.trim().parse::<usize>().ok()?, 
            flst_zaehler: self.flst_zaehler.trim().parse::<usize>().ok()?, 
            flst_nenner: self.flst_nenner.trim().parse::<usize>().ok(), 
        })
    }
}
