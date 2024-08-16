use std::collections::BTreeMap;
use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct NutzungsArt {
    pub wia: String, // Wirtschaftsart
    pub nab: String,
    pub nak: String, 
    pub bez: String,
    pub def: String,
    pub ken: String,
    pub oak: String,
    pub ehb: String,
}

pub type NutzungsArtMap = BTreeMap<String, NutzungsArt>;

fn main() {
    let s = include_str!("./nutzung.json");
    let s = serde_json::from_str::<NutzungsArtMap>(&s).unwrap();
    uneval::to_out_dir(&s, "nutzung.rs").unwrap();
}