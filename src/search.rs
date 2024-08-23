use std::collections::BTreeMap;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct NutzungsArt {
    pub wia: String,
    pub nab: String,
    pub nak: String, 
    pub bez: String,
    pub def: String,
    pub ken: String,
    pub oak: String,
    pub ehb: String,
}

pub type NutzungsArtMap = BTreeMap<String, NutzungsArt>;

pub fn get_map_internal() -> NutzungsArtMap {
    include!(concat!(env!("OUT_DIR"), "/nutzung.rs"))
}

pub fn search_map(term: &str) -> Vec<(String, NutzungsArt)> {
    let map = crate::get_map();
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
    let mut direct_match = BTreeMap::new();
    for (k, v) in map.iter() {
        if k.to_lowercase() == s {
            direct_match.insert(k.clone(), v.clone());
            continue;
        }
    }

    let mut preferred = direct_match.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>();
    preferred.extend(target.iter().map(|(k, v)| (k.clone(), v.clone())));
    preferred
}