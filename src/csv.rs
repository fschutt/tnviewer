use std::collections::BTreeMap;
use serde_derive::{Serialize, Deserialize};

use crate::xlsx::FlstIdParsed;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum CsvDataType {
    Old(BTreeMap<String, Vec<CsvDatensatz>>),
    New(BTreeMap<String, CsvDataFlstInternal>),
}

impl CsvDataType {

    pub fn keys(&self) -> Vec<&String> {
        match self {
            Self::Old(o) => o.keys().collect(),
            Self::New(o) => o.keys().collect(),
        }
    }
    
    pub fn migrate_new(&self) -> Self {
        match self {
            Self::New(n) => Self::New(n.clone()),
            Self::Old(o) => Self::New(o.iter().map(|(k, v)| {
                (k.clone(), CsvDataFlstInternal {
                    eigentuemer: v.iter().map(|c| c.eigentuemer.clone()).collect(),
                    notiz: v.iter().find_map(|s| if s.notiz.is_empty() { Some(s.notiz.clone()) } else { None }).unwrap_or_default(),
                    nutzung: v.iter().find_map(|s| if s.notiz.is_empty() { Some(s.nutzung.clone()) } else { None }).unwrap_or_default(),
                })
            }).collect())
        }
    }

    pub fn get_old_fallback(&self) -> BTreeMap<String, Vec<CsvDatensatz>> {
        match self {
            Self::Old(n) => n.clone(),
            Self::New(o) => o.iter().map(|(k, v)| {
                (k.clone(), v.eigentuemer.iter().map(|e| CsvDatensatz { 
                    nutzung: v.nutzung.clone(),
                    eigentuemer: e.clone(),
                    notiz: v.notiz.clone(),
                }).collect())
            }).collect()
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            CsvDataType::Old(s) => s.is_empty(),
            CsvDataType::New(s) => s.is_empty(),
        }
    }
}

impl Default for CsvDataType {
    fn default() -> Self {
         CsvDataType::New(BTreeMap::default())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct CsvDataFlstInternal {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub eigentuemer: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub nutzung: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub notiz: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "bleibt")]
    Bleibt,
    #[serde(rename = "aenderung-keine-benachrichtigung")]
    AenderungKeineBenachrichtigung,
    #[serde(rename = "aenderung-mit-benachrichtigung")]
    AenderungMitBenachrichtigung,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CsvDatensatz {
    pub eigentuemer: String,
    pub nutzung: String,
    pub notiz: String,
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
        return Ok(CsvDataType::Old(map).migrate_new());
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
        });
    }

    Ok(CsvDataType::Old(map).migrate_new())
}

pub fn search_for_flst_id(csv: &CsvDataType, flst_id: &str) -> Option<(String, Vec<CsvDatensatz>)> {
    let flst_id = flst_id.replace("_", "");
    let parsed = FlstIdParsed::from_str(&flst_id).parse_num()?.format_start_str();
    csv
    .get_old_fallback()
    .iter()
    .find_map(|(k, v)| {
        if k.starts_with(&parsed) { Some((k.clone(), v.clone())) } else { None }
    })
}