use std::collections::BTreeMap;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct NutzungsArt {
    pub nab: String,
    pub nak: String, 
    pub bez: String,
    pub def: String,
    pub ken: String,
    pub oak: String,
    pub ehb: String,
}

pub type NutzungsArtMap = BTreeMap<String, NutzungsArt>;

pub fn search_map(term: &str) -> BTreeMap<String, NutzungsArt> {
    let map: BTreeMap<String, NutzungsArt> = include!(concat!(env!("OUT_DIR"), "/nutzung.rs"));
    let mut target = BTreeMap::new();
    let s = term.to_lowercase();
    for (k, v) in map.iter() {
        if k.to_lowercase().contains(&s) {
            target.insert(k.clone(), v.clone());
            continue;
        }
        if v.bez.to_lowercase().contains(&s) {
            target.insert(k.clone(), v.clone());
            continue;
        }
        if v.def.to_lowercase().contains(&s) {
            target.insert(k.clone(), v.clone());
            continue;
        }
        if v.ehb.to_lowercase().contains(&s) {
            target.insert(k.clone(), v.clone());
            continue;
        }
    }
    target
}