use std::collections::BTreeMap;
use serde_derive::{Serialize, Deserialize};

use crate::xlsx::FlstIdParsed;

pub type CsvDataType = BTreeMap<String, Vec<CsvDatensatz>>;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "bleibt")]
    Bleibt,
    #[serde(rename = "aenderung-keine-benachrichtigung")]
    AenderungKeineBenachrichtigung,
    #[serde(rename = "aenderung-mit-benachrichtigung")]
    AenderungMitBenachrichtigung,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvDatensatz {
    pub eigentuemer: String,
    pub nutzung: String,
    pub notiz: String,
    pub status: Status,
}

pub fn parse_csv(
    csv: &str, 
    id_col: &str, 
    nutzung_col: &str, 
    eigentuemer_col: &str, 
    delimiter: &str, 
    ignore_firstline: bool
) -> Result<CsvDataType, String> {
    
    let mut cells = csv.lines().map(|l| l.split(delimiter).map(String::from).collect::<Vec<_>>()).collect::<Vec<_>>();
    let mut map = BTreeMap::new();
    
    if cells.is_empty() {
        return Ok(map);
    }

    let attributes_in_order = if ignore_firstline { 
        cells.remove(0)
    } else { 
        vec![id_col.to_string(), nutzung_col.to_string(), eigentuemer_col.to_string()] 
    };

    for (i, line) in cells.iter().enumerate() {
        let i = if ignore_firstline { i + 2 } else { i + 1 };
        let mut attributes = BTreeMap::new();
        for (cell, col_name) in line.iter().zip(attributes_in_order.iter()) {
            attributes.insert(col_name.clone(), cell.clone());
        }
        let id = attributes.remove(id_col).ok_or_else(|| format!("Fehler in Zeile {i}: ID fehlt (Spalte {id_col})"))?;
        let eigentuemer = attributes.remove(eigentuemer_col).ok_or_else(|| format!("Fehler in Zeile {i}: Eigentümer fehlt (Spalte {eigentuemer_col})"))?;
        let nutzung = attributes.remove(nutzung_col).ok_or_else(|| format!("Fehler in Zeile {i}: Nutzung fehlt (Spalte {nutzung_col})"))?;

        map.entry(id)
        .or_insert_with(|| Vec::new())
        .push(CsvDatensatz {
            eigentuemer,
            nutzung,
            notiz: String::new(),
            status: Status::Bleibt,
        });
    }

    Ok(map)
}

pub fn search_for_flst_id(csv: &CsvDataType, flst_id: &str) -> Option<(String, Vec<CsvDatensatz>)> {
    let flst_id = flst_id.replace("_", "");
    let parsed = FlstIdParsed::from_str(&flst_id).parse_num()?.format_start_str();
    csv
    .iter()
    .find_map(|(k, v)| {
        if k.starts_with(&parsed) { Some((k.clone(), v.clone())) } else { None }
    })
}