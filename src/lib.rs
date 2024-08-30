use std::collections::{BTreeMap, BTreeSet};

use nas::{intersect_polys, parse_nas_xml, translate_to_geo_poly, NasXMLFile, SplitNasXml, SvgPolygon, TaggedPolygon, LATLON_STRING};
use pdf::{reproject_aenderungen_back_into_latlon, reproject_aenderungen_into_target_space, EbenenStyle, Konfiguration, PdfEbenenStyle, ProjektInfo, RissConfig, RissExtent, RissMap, Risse, StyleConfig};
use proj4rs::proj;
use ui::{Aenderungen, AenderungenIntersection, PolyNeu};
use uuid_wasm::{log_status, log_status_clear};
use wasm_bindgen::prelude::*;
use xml::XmlNode;
use crate::ui::UiData;
use crate::csv::CsvDataType;
use serde_derive::{Serialize, Deserialize};
use web_sys::console::log_1;

pub mod xml;
pub mod ui;
pub mod nas;
pub mod csv;
pub mod xlsx;
pub mod search;
pub mod pdf;
pub mod uuid_wasm;
pub mod zip;
pub mod geograf;
pub mod david;
pub mod optimize;

pub const ARIAL_TTF: &[u8] = include_bytes!("./Arial.ttf");

#[wasm_bindgen]
pub fn get_new_poly_id() -> String {
    crate::uuid_wasm::uuid()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CleanStageResult {
    pub aenderungen: Aenderungen,
    pub log: Vec<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeoJSONResult {
    pub geojson1: String,
    pub geojson2: String,
    pub bounds: [[f64;2];2],
}

#[wasm_bindgen]
pub fn get_problem_geojson() -> String {
    let proj = "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33";
    
    let poly_string1 =   
    "[{\"outer_rings\":[{\"points\":[{\"x\":424728.6,\"y\":5919452.636},{\"x\":424750.775,\"y\":5919462.145},{\"x\":424763.115,\"y\":5919475.823},{\"x\":424771.097,\"y\":5919495.706},{\"x\":424773.6,\"y\":5919516.281},{\"x\":424763.208,\"y\":5919540.72},{\"x\":424751.213,\"y\":5919552.748},{\"x\":424722.988,\"y\":5919566.237},{\"x\":424674.5,\"y\":5919614.52},{\"x\":424652.977,\"y\":5919635.781},{\"x\":424636.974,\"y\":5919649.642},{\"x\":424633.925,\"y\":5919659.739},{\"x\":424636.794,\"y\":5919671.966},{\"x\":424638.73383388907,\"y\":5919675.612687728},{\"x\":424631.31,\"y\":5919691.601},{\"x\":424634.584,\"y\":5919713.918},{\"x\":424693.877,\"y\":5919715.384},{\"x\":424781.572,\"y\":5919777.508},{\"x\":423927.296,\"y\":5919911.944},{\"x\":423924.743,\"y\":5919901.828},{\"x\":424728.6,\"y\":5919452.636}]}],\"inner_rings\":[]},{\"outer_rings\":[{\"points\":[{\"x\":424654.402,\"y\":5919641.869},{\"x\":424638.734,\"y\":5919675.613},{\"x\":424638.73383388907,\"y\":5919675.612687728},{\"x\":424654.402,\"y\":5919641.869}]}],\"inner_rings\":[]}]";

    let poly_string2 =
        "[{\"outer_rings\":[{\"points\":[{\"x\":424654.402,\"y\":5919641.869},{\"x\":424669.268,\"y\":5919623.359},{\"x\":424674.5,\"y\":5919614.52},{\"x\":424722.988,\"y\":5919566.237},{\"x\":424751.213,\"y\":5919552.748},{\"x\":424763.208,\"y\":5919540.72},{\"x\":424773.6,\"y\":5919516.281},{\"x\":424772.498,\"y\":5919462.14},{\"x\":424769.398,\"y\":5919456.498},{\"x\":424750.414,\"y\":5919440.448},{\"x\":424773.939,\"y\":5919438.557},{\"x\":424782.473,\"y\":5919443.365},{\"x\":424786.507,\"y\":5919444.479},{\"x\":424791.694,\"y\":5919443.464},{\"x\":424800.04,\"y\":5919442.007},{\"x\":424808.205,\"y\":5919441.612},{\"x\":424818.371,\"y\":5919443.93},{\"x\":424828.752,\"y\":5919448.584},{\"x\":424836.991,\"y\":5919451.301},{\"x\":424839.133,\"y\":5919448.562},{\"x\":424846.772,\"y\":5919466.244},{\"x\":424845.9,\"y\":5919470.532},{\"x\":424847.102,\"y\":5919475.041},{\"x\":424858.249,\"y\":5919482.578},{\"x\":424858.651,\"y\":5919493.542},{\"x\":424859.158,\"y\":5919507.371},{\"x\":424860.849,\"y\":5919518.113},{\"x\":424863.548,\"y\":5919521.82},{\"x\":424880.069,\"y\":5919518.941},{\"x\":424886.62,\"y\":5919520.532},{\"x\":424898.916,\"y\":5919519.622},{\"x\":424898.795,\"y\":5919519.631},{\"x\":424899.741,\"y\":5919524.26},{\"x\":424900.209,\"y\":5919525.675},{\"x\":424902.338,\"y\":5919525.019},{\"x\":424901.52702871885,\"y\":5919519.42919795},{\"x\":424907.353,\"y\":5919518.999},{\"x\":424909.813,\"y\":5919525.139},{\"x\":424913.175,\"y\":5919533.532},{\"x\":424917.157,\"y\":5919548.815},{\"x\":424919.201,\"y\":5919556.661},{\"x\":424921.434,\"y\":5919565.232},{\"x\":424929.073,\"y\":5919583.243},{\"x\":424930.598,\"y\":5919588.727},{\"x\":424932.659,\"y\":5919593.728},{\"x\":424937.681,\"y\":5919614.77},{\"x\":424938.54,\"y\":5919624.117},{\"x\":424937.734,\"y\":5919635.846},{\"x\":424938.937,\"y\":5919658.754},{\"x\":424939.44,\"y\":5919668.325},{\"x\":424940.735,\"y\":5919680.417},{\"x\":424941.801,\"y\":5919690.373},{\"x\":424942.467,\"y\":5919696.593},{\"x\":424943.243,\"y\":5919703.843},{\"x\":424945.805,\"y\":5919727.764},{\"x\":424948.816,\"y\":5919734.643},{\"x\":424951.525,\"y\":5919743.238},{\"x\":424953.702,\"y\":5919760.784},{\"x\":424954.622,\"y\":5919768.201},{\"x\":424957.656,\"y\":5919778.436},{\"x\":424944.297,\"y\":5919777.829},{\"x\":424931.966,\"y\":5919777.269},{\"x\":424912.058,\"y\":5919773.536},{\"x\":424905.185,\"y\":5919758.054},{\"x\":424898.958,\"y\":5919759.034},{\"x\":424847.745,\"y\":5919767.094},{\"x\":424781.572,\"y\":5919777.508},{\"x\":424813.614,\"y\":5919772.465},{\"x\":424808.792,\"y\":5919770.258},{\"x\":424785.597,\"y\":5919757.82},{\"x\":424758.972,\"y\":5919742.769},{\"x\":424732.614,\"y\":5919722.023},{\"x\":424709.036,\"y\":5919696.786},{\"x\":424676.887,\"y\":5919679.11},{\"x\":424638.693,\"y\":5919675.701},{\"x\":424631.31,\"y\":5919691.601},{\"x\":424654.402,\"y\":5919641.869}]}],\"inner_rings\":[]},{\"outer_rings\":[{\"points\":[{\"x\":424915.748,\"y\":5919578.076},{\"x\":424900.707,\"y\":5919528.958},{\"x\":424903.546,\"y\":5919528.112},{\"x\":424920.754,\"y\":5919578.975},{\"x\":424916.219,\"y\":5919579.67},{\"x\":424915.748,\"y\":5919578.076}]}],\"inner_rings\":[]},{\"outer_rings\":[{\"points\":[{\"x\":424940.756,\"y\":5919750.025},{\"x\":424917.218,\"y\":5919592.104},{\"x\":424922.192,\"y\":5919590.958},{\"x\":424948.679,\"y\":5919766.262},{\"x\":424944.321,\"y\":5919766.865},{\"x\":424940.756,\"y\":5919750.025}]}],\"inner_rings\":[]},{\"outer_rings\":[{\"points\":[{\"x\":424901.527,\"y\":5919519.429},{\"x\":424901.52702871885,\"y\":5919519.42919795},{\"x\":424898.916,\"y\":5919519.622},{\"x\":424901.527,\"y\":5919519.429}]}],\"inner_rings\":[]}]";

    
    let s1 = serde_json::from_str::<Vec<SvgPolygon>>(&poly_string1.trim()).unwrap_or_default();
    let s2 = serde_json::from_str::<Vec<SvgPolygon>>(&poly_string2.trim()).unwrap_or_default();

    let s1: Vec<SvgPolygon> = s1.iter().map(|s| crate::pdf::reproject_poly_back_into_latlon(&s, proj).unwrap_or_default()).collect::<Vec<_>>();
    let s2 = s2.iter().map(|s| crate::pdf::reproject_poly_back_into_latlon(&s, proj).unwrap_or_default()).collect::<Vec<_>>();

    let v1 = s1.iter().map(|t| TaggedPolygon {
        poly: t.clone(),
        attributes: BTreeMap::new(),
    }).collect::<Vec<_>>();

    let v2 = s2.iter().map(|t| TaggedPolygon {
        poly: t.clone(),
        attributes: BTreeMap::new(),
    }).collect::<Vec<_>>();

    serde_json::to_string(&GeoJSONResult {
        geojson1: crate::nas::tagged_polys_to_featurecollection(&v1),
        geojson2: crate::nas::tagged_polys_to_featurecollection(&v2),
        bounds: s2.get(0).unwrap().get_fit_bounds(),
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn lib_nutzungen_saeubern(
    id: Option<String>, 
    aenderungen: String, 
    split_nas_xml: String, 
    nas_original: String,
    konfiguration: String,
) -> String {
    let id = id.and_then(|s| if s.is_empty() { None } else { Some(s.trim().to_string())});

    let aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let split_nas_xml = match serde_json::from_str::<SplitNasXml>(&split_nas_xml) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let nas_original = match serde_json::from_str::<NasXMLFile>(&nas_original) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let konfiguration = match serde_json::from_str::<Konfiguration>(&konfiguration) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &split_nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let mut log = Vec::new();

    log.push(format!("cleaning {} aenderungen, id poly = {id:?}", aenderungen.na_polygone_neu.len()));

    let clean = aenderungen
    .clean_stage1(&mut log, konfiguration.merge.stage1_maxdst_point, konfiguration.merge.stage1_maxdst_line)
    .clean_stage2(&mut log, 1.0, 1.0, 10.0)
    .clean_stage3(&split_nas_xml, &mut log, konfiguration.merge.stage2_maxdst_point, konfiguration.merge.stage2_maxdst_line)
    .clean_stage4(&nas_original, &mut log, konfiguration.merge.stage3_maxdst_line, konfiguration.merge.stage3_maxdst_line2, konfiguration.merge.stage3_maxdeviation_followline);
    
    log.push(format!("cleaned {} aenderungen!", aenderungen.na_polygone_neu.len()));

    let clean = match id {
        Some(s) => match clean.na_polygone_neu.get(&s) {
            Some(q) => {
                let mut aenderungen_clone = aenderungen.clone();
                aenderungen_clone.na_polygone_neu.insert(s.clone(), q.clone());
                aenderungen_clone
            },
            None => aenderungen,
        },
        None => clean,
    };

    let clean = match reproject_aenderungen_back_into_latlon(&clean, &split_nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    serde_json::to_string(&CleanStageResult {
        aenderungen: clean,
        log,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn lib_get_aenderungen_clean(id: Option<String>, aenderungen: Option<String>, split_nas_xml: Option<String>, nas_original: Option<String>, konfiguration: Option<String>) -> String {
    
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen.unwrap_or_default()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };    
    
    let split_nas_xml = serde_json::from_str::<SplitNasXml>(&split_nas_xml.unwrap_or_default()).unwrap_or_default();
    let nas_original = serde_json::from_str::<NasXMLFile>(&nas_original.unwrap_or_default()).unwrap_or_default();
    let konfiguration = serde_json::from_str::<Konfiguration>(&konfiguration.unwrap_or_default()).unwrap_or_default();

    let aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &split_nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let mut log = Vec::new();
    let id = id.unwrap_or_default();

    log.push(format!("cleaning {} aenderungen, stage = {id}", aenderungen.na_polygone_neu.len()));

    let clean = match id.as_str() {
        "1" => aenderungen.clean_stage1(&mut log, konfiguration.merge.stage1_maxdst_point, konfiguration.merge.stage1_maxdst_line),
        "2" => aenderungen.clean_stage2(&mut log, 1.0, 1.0, 10.0),
        "25" => aenderungen.clean_stage25(),
        "3" => aenderungen.clean_stage3(&split_nas_xml, &mut log, konfiguration.merge.stage2_maxdst_point, konfiguration.merge.stage2_maxdst_line),
        "4" => aenderungen.clean_stage4(&nas_original, &mut log, konfiguration.merge.stage3_maxdst_line, konfiguration.merge.stage3_maxdst_line2, konfiguration.merge.stage3_maxdeviation_followline),
        "13" => {
            aenderungen
            .clean_stage1(&mut log, konfiguration.merge.stage1_maxdst_point, konfiguration.merge.stage1_maxdst_line)
            .clean_stage2(&mut log, 1.0, 1.0, 10.0)
            .clean_stage3(&split_nas_xml, &mut log, konfiguration.merge.stage2_maxdst_point, konfiguration.merge.stage2_maxdst_line)
            .clean_stage4(&nas_original, &mut log, konfiguration.merge.stage3_maxdst_line, konfiguration.merge.stage3_maxdst_line2, konfiguration.merge.stage3_maxdeviation_followline)
        },
        "5" => aenderungen.clean_stage5(&split_nas_xml, &mut log),
        "6" => aenderungen.clean_stage6(&split_nas_xml, &nas_original, &mut log),
        "8" => aenderungen.clean_stage7_test(&split_nas_xml, &nas_original, &mut log, &konfiguration),
        _ => return format!("wrong id {id}"),
    };

    log.push(format!("cleaned {} aenderungen!", aenderungen.na_polygone_neu.len()));

    let clean = match reproject_aenderungen_back_into_latlon(&clean, &split_nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    serde_json::to_string(&CleanStageResult {
        aenderungen: clean,
        log,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn aenderungen_zu_geograf(
    split_nas_xml: String,
    nas_xml: String,
    projekt_info: String,
    konfiguration: String,
    aenderungen: String,
    risse: String,
    risse_extente: String,
    csv_data: String,
) -> Vec<u8> {

    log_status_clear();
    log_status("Starte Export nach GEOgraf...");
    let split_nas_xml = match serde_json::from_str::<SplitNasXml>(split_nas_xml.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let nas_xml = match serde_json::from_str::<NasXMLFile>(nas_xml.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let projekt_info = match serde_json::from_str::<ProjektInfo>(projekt_info.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let konfiguration = match serde_json::from_str::<Konfiguration>(konfiguration.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let risse = match serde_json::from_str::<Risse>(&risse) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let risse_extente = match serde_json::from_str::<RissMap>(&risse_extente) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };

    let csv_data = match serde_json::from_str::<CsvDataType>(&csv_data) {
        Ok(o) => o,
        Err(_) => BTreeMap::default(),
    };

    let result = std::panic::catch_unwind(|| {
        crate::geograf::export_aenderungen_geograf(
            &split_nas_xml,
            &nas_xml,
            &projekt_info,
            &konfiguration,
            &aenderungen,
            &risse,
            &risse_extente,
            &csv_data,
        )
    });

    match result {
        Ok(o) => o,
        Err(e) => {
            let s = format!("FEHLER: {:?}", e);
            log_status(&s);
            s.as_bytes().to_vec()
        },
    }
}


#[wasm_bindgen]
pub fn aenderungen_zu_david(aenderungen: String, split_nas_xml: String) -> String {
    let aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let xml = match serde_json::from_str::<SplitNasXml>(&split_nas_xml) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    crate::david::aenderungen_zu_fa_xml(&aenderungen, &xml)
}

#[wasm_bindgen]
pub fn get_geojson_fuer_neue_polygone(aenderungen: String) -> String {
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct NeuePolygoneGeoJson {
        nutzung_definiert: bool,
        geojson: String,
    }

    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();

    let construct_polys = |(k, v): (&String, &PolyNeu)| {
        TaggedPolygon {
            attributes: vec![("newPolyId".to_string(), k.clone())].into_iter().collect(),
            poly: v.poly.clone(),
        }
    };

    let nutzung_definiert = aenderungen.na_polygone_neu.iter()
    .filter(|(_, poly)| poly.nutzung.is_some())
    .map(construct_polys).collect::<Vec<_>>();

    let nutzung_definiert = NeuePolygoneGeoJson {
        nutzung_definiert: true,
        geojson: crate::nas::tagged_polys_to_featurecollection(&nutzung_definiert),
    };

    let nutzung_nicht_definiert = aenderungen.na_polygone_neu.iter()
    .filter(|(_, poly)| poly.nutzung.is_none())
    .map(construct_polys).collect::<Vec<_>>();

    let nutzung_nicht_definiert = NeuePolygoneGeoJson {
        nutzung_definiert: false,
        geojson: crate::nas::tagged_polys_to_featurecollection(&nutzung_nicht_definiert),
    };

    serde_json::to_string(&[
        nutzung_definiert,
        nutzung_nicht_definiert
    ]).unwrap_or_default()

}

#[wasm_bindgen]
pub fn get_polyline_guides_in_current_bounds(
    split_flurstuecke: String,
    aenderungen: String,
    map_bounds: String,
) -> String {

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
    struct MapBounds {
        _northEast: crate::LatLng,
        _southWest: crate::LatLng,
    }
    
    let mut pl = Vec::new();
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();
    match serde_json::from_str::<MapBounds>(&map_bounds) {
        Ok(MapBounds { _northEast, _southWest }) => {

            let rect = quadtree_f32::Rect {
                min_x: _southWest.lng,
                min_y: _southWest.lat,
                max_x: _northEast.lng,
                max_y: _northEast.lat,
            };
            let mut ringe = split_fs.get_polyline_guides_in_bounds(rect);
            let mut aenderungen_ringe = aenderungen.na_polygone_neu.values().flat_map(|p| {
                let mut v = p.poly.outer_rings.clone();
                v.append(&mut p.poly.inner_rings.clone());
                v.into_iter()
            }).collect::<Vec<_>>();
            ringe.append(&mut aenderungen_ringe);
            pl = ringe.iter().map(|svg_line| {
                svg_line.points.iter().map(|p| [p.y, p.x]).collect::<Vec<_>>()
            }).collect::<Vec<_>>();
        },
        Err(e) => { },
    }
    serde_json::to_string(&pl).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

#[wasm_bindgen]
pub fn fixup_polyline(
    xml: String,
    split_flurstuecke: String,
    points: String,
) -> String {

    use crate::nas::SplitNasXml;
    use crate::nas::SvgLine;
    use crate::nas::SvgPoint;

    let xml = serde_json::from_str::<NasXMLFile>(&xml).unwrap_or_default();
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    let points = serde_json::from_str::<Vec<LatLng>>(&points).unwrap_or_default();

    fn fixup_polyline_internal(points: &[LatLng], split_fs: &SplitNasXml) -> Option<SvgPolygon> {
        
        let mut points = points.to_vec();
        
        if points.first()? != points.last()? {
            points.push(points.first()?.clone());
        }

        Some(SvgPolygon {
            outer_rings: vec![SvgLine {
                points: points.iter().map(|p| {
                    SvgPoint {
                        x: p.lng,
                        y: p.lat,
                    }
                }).collect(),
            }],
            inner_rings: Vec::new()
        })
    }

    let poly = match fixup_polyline_internal(&points, &split_fs) {
        Some(s) => s,
        None => return format!("failed to create poly from points {points:?}"),
    };

    serde_json::to_string(&crate::ui::PolyNeu {
        poly: poly,
        nutzung: None,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn ui_render_entire_screen(
    projektinfo: String,
    risse: String,
    uidata: String, 
    csv: String, 
    aenderungen: String,
    konfiguration: String,
) -> String {
    let projektinfo = serde_json::from_str::<ProjektInfo>(&projektinfo).unwrap_or_default();
    let risse = serde_json::from_str::<Risse>(&risse).unwrap_or_default();
    let uidata = UiData::from_string(&uidata);
    let csv = serde_json::from_str(&csv).unwrap_or_default();
    let aenderungen = serde_json::from_str(&aenderungen).unwrap_or_default();
    let konfiguration = serde_json::from_str::<Konfiguration>(&konfiguration).unwrap_or_default();
    crate::ui::render_entire_screen(&projektinfo, &risse, &uidata, &csv, &aenderungen, &konfiguration)
}

#[wasm_bindgen]
pub fn ui_render_ribbon(decoded: String) -> String {
    let uidata = UiData::from_string(&decoded);
    crate::ui::render_ribbon(&uidata, false)
}


#[wasm_bindgen]
pub fn ui_render_search_popover_content(search_term: String) -> String {
    crate::ui::ui_render_search_popover_content(&search_term)
}

#[wasm_bindgen]
pub fn ui_render_popover_content(decoded: String, konfiguration: String) -> String {
    let uidata = UiData::from_string(&decoded);
    let konfiguration = match serde_json::from_str(&konfiguration) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    crate::ui::render_popover_content(&uidata, &konfiguration)
}

#[wasm_bindgen]
pub fn ui_render_switch_content(
    uidata: String, 
) -> String {
    let uidata = UiData::from_string(&uidata);
    crate::ui::render_switch_content(&uidata)
}

#[wasm_bindgen]
pub fn ui_render_project_content(
    projektinfo: String,
    risse: String,
    uidata: String, 
    csv_data: String, 
    aenderungen: String, 
    split_flurstuecke: Option<String>
) -> String {
    let projektinfo = serde_json::from_str::<ProjektInfo>(&projektinfo).unwrap_or_default();
    let risse = serde_json::from_str::<Risse>(&risse).unwrap_or_default();
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let uidata = UiData::from_string(&uidata);
    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or(CsvDataType::default());
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke.unwrap_or_default()).unwrap_or_default();
    if uidata.secondary_content.unwrap_or_default() {
        crate::ui::render_secondary_content(&aenderungen)
    } else {
        crate::ui::render_project_content(&projektinfo, &risse, &csv_data, &aenderungen, &uidata, &split_fs)
    }
}

#[wasm_bindgen]
pub fn get_geojson_polygon(s: String) -> String {
    let flst = match serde_json::from_str::<TaggedPolygon>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string()
    };
    crate::nas::tagged_polys_to_featurecollection(&[flst])
}

#[wasm_bindgen]
pub fn get_fit_bounds(s: String) -> String {
    let flst = match serde_json::from_str::<TaggedPolygon>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string()
    };
    let bounds = flst.get_fit_bounds();
    serde_json::to_string(&bounds).unwrap_or_default()
}

#[wasm_bindgen]
pub fn search_for_polyneu(aenderungen: String, poly_id: String) -> String {
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let tp = aenderungen.na_polygone_neu.iter()
    .find_map(|(k, v)| if k.as_str() == poly_id.as_str() {
        Some(TaggedPolygon {
            attributes: BTreeMap::new(),
            poly: v.poly.clone(),
        })
    } else {
        None
    });

    match tp {
        Some(s) => serde_json::to_string(&s).unwrap_or_default(),
        None => String::new(),
    }
}

#[wasm_bindgen]
pub fn search_for_gebauede(s: String, gebaeude_id: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let ax_gebauede = match xml.ebenen.get("AX_Gebaeude") {
        Some(s) => s,
        None => return "keine Ebene AX_Gebaeude".to_string(),
    };
    let r =  ax_gebauede
    .iter()
    .find(|f| f.attributes.get("id") == Some(&gebaeude_id));
    
    match r {
        Some(s) => serde_json::to_string(&s).unwrap_or_default(),
        None => String::new(),
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LoadNasReturn {
    pub log: Vec<String>,
    pub xml_parsed: Vec<XmlNode>,
    pub nas_original: NasXMLFile,
    pub nas_cut_original: SplitNasXml,
    pub nas_projected: NasXMLFile,
    pub nas_cut_projected: SplitNasXml,
}

#[wasm_bindgen]
pub fn get_ebenen_darstellung(
    konfiguration: String,
) -> String {
    let konfiguration = match serde_json::from_str::<Konfiguration>(&konfiguration) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let arr = konfiguration.style.get_styles_sorted().iter().map(|(k, v)| {
        v.name.clone()
    }).collect::<Vec<_>>();
    serde_json::to_string(&arr).unwrap_or_default()
}

#[derive(Debug, Serialize, Deserialize)]
struct NasParseError {
    error: String, 
    log: Vec<String>,
}

#[wasm_bindgen]
pub fn load_nas_xml(s: String, style: String) -> String {
    let konfiguration = match serde_json::from_str::<Konfiguration>(&style) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let mut t = konfiguration.style.ebenen.values().map(|s| s.name.clone()).collect::<Vec<_>>();
    t.sort();
    t.dedup();

    let mut log = Vec::new();
    log.push(format!("parsing XML: types = {t:?}"));

    let xml_parsed = match crate::xml::parse_xml_string(&s, &mut log) {
        Ok(o) => o,
        Err(e) => return serde_json::to_string(&NasParseError {
            error: format!("XML parse error: {e:?}"),
            log: log,
        }).unwrap_or_default(),
    };
    let nas_original = match crate::nas::parse_nas_xml(xml_parsed.clone(), &t, &mut log) {
        Ok(o) => o,
        Err(e) => return serde_json::to_string(&NasParseError {
            error: e,
            log: log,
        }).unwrap_or_default(),
    };
    let nas_cut_original = match crate::nas::split_xml_flurstuecke_inner(&nas_original, &mut log) {
        Ok(o) => o,
        Err(e) => return serde_json::to_string(&NasParseError {
            error: e,
            log: log,
        }).unwrap_or_default(),
    };
    let nas_projected = match crate::nas::transform_nas_xml_to_lat_lon(&nas_original, &mut log) {
        Ok(o) => o,
        Err(e) => return serde_json::to_string(&NasParseError {
            error: e,
            log: log,
        }).unwrap_or_default(),
    };
    let mut nas_cut_projected = match crate::nas::transform_split_nas_xml_to_lat_lon(&nas_cut_original, &mut log) {
        Ok(o) => o,
        Err(e) => return serde_json::to_string(&NasParseError {
            error: e,
            log: log,
        }).unwrap_or_default(),
    };
    crate::nas::fixup_flst_groesse(&nas_cut_original, &mut nas_cut_projected);
    serde_json::to_string(&LoadNasReturn {
        log,
        xml_parsed,
        nas_original,
        nas_cut_original,
        nas_projected,
        nas_cut_projected,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn get_geojson_fuer_ebene(json: String, layer: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(e) => return "ERROR HERE".to_string() + &e.to_string(),
    };
    xml.get_geojson_ebene(&layer)
}

#[wasm_bindgen]
pub fn get_layer_style(konfiguration: String, layer_name: String) -> String {
    let konfiguration = match serde_json::from_str::<Konfiguration>(&konfiguration) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let ls = konfiguration.style.ebenen.iter()
    .find(|(_, s)| s.name.trim() == layer_name.trim())
    .map(|(_, v)| v.clone());
    let ls = match ls {
        Some(s) => s,
        None => return format!("style für {layer_name} nicht gefunden"),
    };
    serde_json::to_string(&ls).unwrap_or_default()
}

#[wasm_bindgen]
pub fn get_gebaeude_geojson_fuer_aktive_flst(json: String, csv: String, aenderungen: String) -> String {
    let default = format!("{{ \"type\": \"FeatureCollection\", \"features\": [] }}");
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(_) => return default.clone(),
    };
    let csv = match serde_json::from_str::<CsvDataType>(&csv) {
        Ok(o) => o,
        Err(_) => return default.clone(),
    };
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(_) => return default.clone(),
    };
    xml.get_gebaeude(&csv, &aenderungen)
}

#[wasm_bindgen]
pub fn get_labels_fuer_ebene(json: String, layer: String) -> String {
    let xml = match serde_json::from_str::<NasXMLFile>(&json) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let labels = xml.get_geojson_labels(&layer);
    serde_json::to_string(&labels).unwrap_or_default()
}


#[wasm_bindgen]
pub fn parse_csv_dataset_to_json(
    csv: Vec<u8>, 
    id_col: String, 
    nutzung_col: String, 
    eigentuemer_col: String, 
    delimiter: String,
    ignore_firstline: String
) -> String {
    let csv = decode(csv);
    let csv_daten = match crate::csv::parse_csv(
        &csv, 
        &id_col, 
        &nutzung_col, 
        &eigentuemer_col, 
        &delimiter,
        ignore_firstline == "true"
    ) {
        Ok(o) => o,
        Err(e) => return e,
    };
    serde_json::to_string(&csv_daten).unwrap_or_default()
}

#[wasm_bindgen]
pub fn export_alle_flst(s: String) -> String {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    crate::xlsx::get_alle_flst(&data)
}

#[wasm_bindgen]
pub fn export_xlsx(s: String) -> Vec<u8> {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    crate::xlsx::generate_report(&data)
}

#[derive(Debug, Serialize, Deserialize)]
struct KonfigurationLayerAlle {
    pub result: Konfiguration, 
    pub log: Vec<String>,
}

#[wasm_bindgen]
pub fn edit_konfiguration_layer_alle(konfiguration: String, xml_nas: String) -> String {

    let mut config = serde_json::from_str::<Konfiguration>(&konfiguration).unwrap_or_default();
    let nas_projected = serde_json::from_str::<Vec<XmlNode>>(&xml_nas).unwrap_or_default();

    let mut log = Vec::new();

    let kuerzel = crate::xml::get_all_nodes_in_tree(&nas_projected)
        .iter()
        .filter(|n| n.node_type.starts_with("AX_"))
        .map(|n| n.node_type.clone())
        .collect::<Vec<_>>();

    log.push(format!("alle_ax: {:?}", kuerzel));

    let nas_parsed_complete = match parse_nas_xml(nas_projected, &kuerzel, &mut Vec::new()) {
        Ok(s) => s,
        Err(_) => NasXMLFile::default(),
    };

    let tp_count = nas_parsed_complete.ebenen.iter()
    .map(|(k, s)| (k.clone(), s.len()))
    .collect::<BTreeMap<_, _>>(); 
    log.push(format!("tp_count: {:?}", tp_count));

    let alle_auto_kuerzel = nas_parsed_complete.ebenen.iter().flat_map(|(k, s)| {
        s.into_iter().filter_map(|tp| tp.get_auto_kuerzel(k))
    }).collect::<BTreeSet<_>>();

    let neue_ebenen = alle_auto_kuerzel.into_iter().map(|ak| {
        (get_new_poly_id(), {
            let mut m = PdfEbenenStyle::default();
            m.kuerzel = ak;
            m.fill_color = Some(crate::uuid_wasm::random_color());
            m
        })
    }).collect::<Vec<_>>();

    config.pdf.nutzungsarten = neue_ebenen.iter().cloned().collect();
    config.pdf.layer_ordnung = neue_ebenen.iter().map(|(k, _)| k.clone()).collect();
    serde_json::to_string(&KonfigurationLayerAlle {
        result: config,
        log,
    }).unwrap_or_default()
}

#[wasm_bindgen]
pub fn edit_konfiguration_layer_neu(konfiguration: String, layer_type: String) -> String {
    let mut config = serde_json::from_str::<Konfiguration>(&konfiguration).unwrap_or_default();
    match layer_type.as_str() {
        "style" => {
            let ebene_id = get_new_poly_id();
            config.style.ebenen.insert(ebene_id.clone(), EbenenStyle::default());
            let mut copy = config.style.ebenen_ordnung.clone();
            copy.reverse();
            copy.push(ebene_id);
            copy.reverse();
            config.style.ebenen_ordnung = copy;
        },
        "pdf-nutzungsarten" =>  {
            let ebene_id = get_new_poly_id();
            config.pdf.nutzungsarten.insert(ebene_id.clone(), PdfEbenenStyle::default());
            let mut copy = config.pdf.layer_ordnung.clone();
            copy.reverse();
            copy.push(ebene_id);
            copy.reverse();
            config.pdf.layer_ordnung = copy;
        },
        _ => { },
    }
    serde_json::to_string(&config).unwrap_or_default()
}

#[wasm_bindgen]
pub fn edit_konfiguration_move_layer(konfiguration: String, layer_type: String, ebene_id: String, move_type: String) -> String {
    let mut config = serde_json::from_str::<Konfiguration>(&konfiguration).unwrap_or_default();
    match move_type.as_str() {
        "delete" => {
            match layer_type.as_str() {
                "style" => {
                    config.style.ebenen.remove(ebene_id.as_str());
                    config.style.ebenen_ordnung.retain(|s| *s != ebene_id);
                },
                "pdf-nutzungsarten" => {
                    config.pdf.nutzungsarten.remove(ebene_id.as_str());
                    config.pdf.layer_ordnung.retain(|s| *s != ebene_id);
                },
                _ => { },
            }
        },
        "move-up" =>  {
            match layer_type.as_str() {
                "style" => {
                    let mut temp = ebene_id.clone();
                    if let Some(pos) = config.style.ebenen_ordnung.iter().position(|s| s.as_str() == ebene_id) {
                        if let Some(next) = config.style.ebenen_ordnung.get_mut(pos.saturating_sub(1)) {
                            std::mem::swap(&mut temp, next);
                        }
                        if let Some(pos_st) = config.style.ebenen_ordnung.get_mut(pos) {
                            std::mem::swap(&mut temp, pos_st);
                        }
                    }
                },
                "pdf-nutzungsarten" => {
                    let mut temp = ebene_id.clone();
                    if let Some(pos) = config.pdf.layer_ordnung.iter().position(|s| s.as_str() == ebene_id) {
                        if let Some(next) = config.pdf.layer_ordnung.get_mut(pos.saturating_sub(1)) {
                            std::mem::swap(&mut temp, next);
                        }
                        if let Some(pos_st) = config.pdf.layer_ordnung.get_mut(pos) {
                            std::mem::swap(&mut temp, pos_st);
                        }
                    }
                },
                _ => { },
            }
        },
        "move-down" =>  {
            match layer_type.as_str() {
                "style" => {
                    let mut temp = ebene_id.clone();
                    if let Some(pos) = config.style.ebenen_ordnung.iter().position(|s| s.as_str() == ebene_id) {
                        if let Some(next) = config.style.ebenen_ordnung.get_mut(pos.saturating_add(1)) {
                            std::mem::swap(&mut temp, next);
                        }
                        if let Some(pos_st) = config.style.ebenen_ordnung.get_mut(pos) {
                            std::mem::swap(&mut temp, pos_st);
                        }
                    }
                },
                "pdf-nutzungsarten" => {
                    let mut temp = ebene_id.clone();
                    if let Some(pos) = config.pdf.layer_ordnung.iter().position(|s| s.as_str() == ebene_id) {
                        if let Some(next) = config.pdf.layer_ordnung.get_mut(pos.saturating_add(1)) {
                            std::mem::swap(&mut temp, next);
                        }
                        if let Some(pos_st) = config.pdf.layer_ordnung.get_mut(pos) {
                            std::mem::swap(&mut temp, pos_st);
                        }
                    }
                },
                _ => { },
            }
        },
        _ => { },
    }
    serde_json::to_string(&config).unwrap_or_default()
}

#[wasm_bindgen]
pub fn export_flst_id_nach_eigentuemer(s: String) -> Vec<u8> {
    let data = match serde_json::from_str::<CsvDataType>(&s) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    crate::xlsx::flst_id_nach_eigentuemer(&data).1
}

pub fn get_map() -> BTreeMap<String, crate::search::NutzungsArt> {
    crate::search::get_map_internal()
}

pub fn decode(bytes: Vec<u8>) -> String {
    let mut text_decoder = chardetng::EncodingDetector::new();
    let _ = text_decoder.feed(&bytes[..], true);
    let text_decoder = text_decoder.guess(None, true);
    let mut text_decoder = text_decoder.new_decoder();
    let mut decoded = String::with_capacity(bytes.len() * 2);
    let _ = text_decoder.decode_to_string(&bytes[..], &mut decoded, true);
    decoded
}