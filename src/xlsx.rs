
use std::collections::BTreeMap;

use serde_derive::Deserialize;
use serde_derive::Serialize;

use crate::csv::CsvDataType;
use crate::csv::Status;

pub fn get_alle_flst(datensaetze: &CsvDataType) -> String {
    let all = get_alle_flst_internal(datensaetze);

    let alle_flst = all.iter()
    .map(|(v, _)| v.clone())
    .collect::<Vec<_>>();
    
    let veraendert = all.iter()
    .filter_map(|(v, b)| if *b {
        Some(v.clone())
    } else {
        None
    }).collect::<Vec<FlstIdParsedNumber>>();

    let s1 = format_flst_liste(alle_flst);
    let s2 = format_flst_liste(veraendert);

    format!("FLURSTUECKE:\r\n{s1}\r\n\r\nVERAENDERT:\r\n\r\n{s2}\r\n")
}

fn format_flst_liste(all: Vec<FlstIdParsedNumber>) -> String {

    let mut grouped = BTreeMap::new();
    for f in all {
        grouped.entry(f.flur).or_insert_with(|| Vec::new()).push(f.clone());
    }

    if grouped.len() == 1 {
        grouped
        .values()
        .next()
        .unwrap()
        .iter()
        .map(|f: &FlstIdParsedNumber| f.format_str_zero())
        .collect::<Vec<_>>()
        .join(",")
        .into()
    } else {
        grouped.iter().map(|(k, v)| {
            format!("FLUR {k}: {}", v.iter()
            .map(|f: &FlstIdParsedNumber| f.format_str_zero())
            .collect::<Vec<_>>()
            .join(",")
        )}).collect::<Vec<String>>()
        .join("\r\n")
    }
}

fn get_alle_flst_internal(datensaetze: &CsvDataType) -> Vec<(FlstIdParsedNumber, bool)> {
    // 12 1175 003 00038 00000
    let mut target = Vec::new();
    for (ds, v) in datensaetze.iter() {

        let ds_modified = v
        .get(0)
        .map(|s| s.status == Status::AenderungMitBenachrichtigung)
        .unwrap_or(false);

        let flst = FlstIdParsed::from_str(ds);
        let flst_num = match flst.parse_num() {
            Some(s) => s,
            None => continue,
        };

        target.push((flst_num, ds_modified));
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

pub fn flst_id_nach_eigentuemer(datensaetze: &CsvDataType) -> (usize, Vec<u8>) {

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
            let flst = match FlstIdParsed::from_str(k).parse_num() {
                Some(s) => s,
                None => continue,
            };
            if d.status != Status::AenderungMitBenachrichtigung {
                continue;
            }
            eigentuemer
            .entry(d.eigentuemer.trim().to_string())
            .or_insert_with(|| BTreeMap::new())
            .entry(flst.flur)
            .or_insert_with(|| Vec::new())
            .push(flst);
        }
    }

    let mut eigentuemer_len = eigentuemer.len();

    let _ = wb.write_sheet(&mut sheet, |sheet_writer| {
        let sw = sheet_writer;
        sw.append_row(row!["Eigentümer", "Flurstücke"])?;

        for (k, v) in eigentuemer.iter() {
            let mut txt = String::new();
            for (flur, fl) in v.iter() {
                let mut v = fl.clone();
                v.sort_by(|a, b| a.flst_zaehler.cmp(&b.flst_zaehler));
                v.dedup();
                let s_flur = v.iter().map(|q| q.format_str()).collect::<Vec<_>>().join(", ");
                txt.push_str(&format!("Fl. {flur}: Flst. {s_flur}"));
                txt.push_str("\r\n");
            }
            sw.append_row(row![
                k.to_string(),
                txt
            ])?;
        }

        Ok(())
    });

    match wb.close() {
        Ok(Some(o)) => (eigentuemer_len, o),
        _ => (0, Vec::new()),
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
pub struct FlstIdParsed {
    pub land: String, 
    pub gemarkung: String,
    pub flur: String,
    pub flst_zaehler: String,
    pub flst_nenner: String,
    pub padding: String,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Serialize, Deserialize)]
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

    pub fn format_nice(&self) -> String {
        let FlstIdParsedNumber { land, gemarkung, flur, flst_zaehler, flst_nenner } = self;
        // 12-1180-003-00001-0000/00
        let n = flst_nenner.unwrap_or(0);
        format!("{land:02}-{gemarkung:04}-{flur:03}-{flst_zaehler:05}/{n:05}")
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
    pub fn format_str_zero(&self) -> String {
        let nenner = self.flst_nenner.unwrap_or(0);
        format!("{}/{}", self.flst_zaehler, nenner)
     }
}

#[test]
fn test_flst_parsing() {
    let s1 = FlstIdParsed::from_str("12 1180 003 00010 0000 00");
    let s2 = FlstIdParsed::from_str("12118000300010______");
    let target = FlstIdParsed {
        land: "12".to_string(),
        gemarkung: "1180".to_string(),
        flur: "003".to_string(),
        flst_zaehler: "00010".to_string(),
        flst_nenner: "0000".to_string(),
        padding: "00".to_string(),
    };
    assert_eq!(s1, s2);
    assert_eq!(s1, target);
}

impl FlstIdParsed {

    pub fn from_str(id: &str) -> FlstIdParsed {

        // 12 1180 003 00001 0000 00
        // 12-1180-003-00261/0000
        let id = id.replace("-", "").replace("/", "").replace(" ", "").replace("_", "");
        let chars = id.chars().collect::<Vec<_>>();
        
        let mut land = chars.iter().skip(0).take(2).collect::<String>();
        match land.parse::<usize>() {
            Ok(s) => { land = format!("{s:02}"); },
            Err(_) => { land = "12".to_string(); },
        }

        let mut gemarkung = chars.iter().skip(2).take(4).collect::<String>();
        match gemarkung.parse::<usize>() {
            Ok(s) => { gemarkung = format!("{s:04}"); },
            Err(_) => { gemarkung = "0000".to_string(); },
        }

        let mut flur = chars.iter().skip(6).take(3).collect::<String>();
        match flur.parse::<usize>() {
            Ok(s) => { flur = format!("{s:03}"); },
            Err(_) => { flur = "001".to_string(); },
        }

        let mut flst_zaehler = chars.iter().skip(9).take(5).collect::<String>();
        match flst_zaehler.parse::<usize>() {
            Ok(s) => { flst_zaehler = format!("{s:05}"); },
            Err(_) => { flst_zaehler = "00001".to_string(); },
        }
        
        let mut flst_nenner = chars.iter().skip(14).take(4).collect::<String>();
        match flst_nenner.parse::<usize>() {
            Ok(s) => { flst_nenner = format!("{s:04}"); },
            Err(_) => { flst_nenner = "0000".to_string(); },
        }

        let mut padding = chars.iter().skip(18).take(2).collect::<String>();
        if padding.is_empty() {
            padding = "00".to_string();
        }

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
            flst_nenner: self.flst_nenner.trim().parse::<usize>().ok().and_then(|s| if s == 0 { None } else { Some(s) }), 
        })
    }
}
