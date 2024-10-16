use crate::{
    csv::CsvDataType,
    ui::UiData,
};
use nas::{
    default_etrs33, parse_nas_xml, NasXMLFile, NasXmlObjects, SplitNasXml, SvgLine, SvgPoint, SvgPolygon, SvgPolygonInner, TaggedPolygon
};
use pdf::{
    reproject_aenderungen_back_into_latlon,
    reproject_aenderungen_into_target_space,
    reproject_point_back_into_latlon,
    EbenenStyle,
    Konfiguration,
    PdfEbenenStyle,
    ProjektInfo,
    RissConfig,
    Risse,
};
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::collections::{
    BTreeMap,
    BTreeSet,
};
use ui::{
    Aenderungen,
    PolyNeu,
};
use uuid_wasm::{
    log_status,
    log_status_clear,
};
use wasm_bindgen::prelude::*;
use xlsx::FlstIdParsed;
use xml::XmlNode;

pub mod csv;
pub mod david;
pub mod geograf;
pub mod nas;
pub mod optimize;
pub mod pdf;
pub mod process;
pub mod search;
pub mod ui;
pub mod uuid_wasm;
pub mod xlsx;
pub mod xml;
pub mod xml_templates;
pub mod zip;
pub mod ops;

pub const ARIAL_TTF: &[u8] = include_bytes!("./Arial.ttf");

#[wasm_bindgen]
pub fn get_new_poly_id() -> String {
    crate::uuid_wasm::uuid()
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveFile {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    crs: Option<String>,
    info: ProjektInfo,
    risse: Risse,
    csv: CsvDataType,
    aenderungen: Aenderungen,
}

#[wasm_bindgen]
pub fn lib_parse_savefile(savefile: String) -> String {
    let sf = match serde_json::from_str::<SaveFile>(&savefile) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let source_crs = sf.crs.clone().unwrap_or(default_etrs33());
    serde_json::to_string(&SaveFile {
        crs: sf.crs.clone(),
        info: sf.info,
        csv: sf.csv.clone(),
        risse: sf
            .risse
            .iter()
            .map(|(k, v)| (k.clone(), v.migrate_old(&source_crs)))
            .collect(),
        aenderungen: sf.aenderungen.migrate_old(&source_crs),
    })
    .unwrap_or_default()
}

#[wasm_bindgen]
pub fn format_savefile(
    info: String,
    risse: Option<String>,
    csv: Option<String>,
    aenderungen: Option<String>,
    target_crs: Option<String>,
) -> String {
    let info = serde_json::from_str::<ProjektInfo>(info.as_str()).unwrap_or_default();
    let risse = serde_json::from_str::<Risse>(&risse.unwrap_or_default()).unwrap_or_default();
    let risse = crate::pdf::reproject_rissgebiete_into_target_space(&risse, &target_crs.clone().unwrap_or(crate::nas::default_etrs33()));
    let csv = serde_json::from_str::<CsvDataType>(&csv.unwrap_or_default()).unwrap_or_default();
    let aenderungen =
        serde_json::from_str::<Aenderungen>(&aenderungen.unwrap_or_default()).unwrap_or_default();
    let aenderungen = target_crs.clone()
    .and_then(|s| reproject_aenderungen_into_target_space(&aenderungen, &s).ok())
    .unwrap_or(aenderungen)
    .migrate_new();
    
    let savefile = SaveFile {
        crs: target_crs.clone(),
        info,
        risse: risse
            .iter()
            .map(|(k, v)| (k.clone(), v.migrate_new()))
            .collect(),
        csv: csv.migrate_new(),
        aenderungen: aenderungen,
    };

    serde_json::to_string_pretty(&savefile).unwrap_or_default()
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
    pub bounds: [[f64; 2]; 2],
}

pub fn get_main_gemarkung(csv: &CsvDataType) -> usize {
    for flst_id in csv.keys() {
        if let Some(f) = FlstIdParsed::from_str(&flst_id)
            .parse_num()
            .map(|f| f.gemarkung)
        {
            return f;
        }
    }
    0
}

#[wasm_bindgen]
pub fn get_rissgebiet_geojson(poly: String, target_crs: String) -> String {
    let s1 = serde_json::from_str::<SvgPolygonInner>(&poly.trim()).unwrap_or_default();
    let s1 = reproject_poly_back_into_latlon(s1, &target_crs);
    let v1 = vec![TaggedPolygon {
        poly:  s1.clone(),
        attributes: BTreeMap::new(),
    }];
    crate::nas::tagged_polys_to_featurecollection(&v1)
}

fn reproject_poly_back_into_latlon(rissgebiet: SvgPolygonInner, crs: &str) -> SvgPolygonInner {

    let already_reprojected = rissgebiet.outer_ring.points.iter().any(|s| s.x < 1000.0 || s.y < 1000.0);
    
    if already_reprojected {
        return rissgebiet;
    }
    
    let latlon_proj = match proj4rs::Proj::from_proj_string(crate::nas::LATLON_STRING) {
        Ok(o) => o,
        Err(_) => return rissgebiet,
    };

    let target_proj = match proj4rs::Proj::from_proj_string(&crs) {
        Ok(o) => o,
        Err(_) => return rissgebiet,
    };

    crate::nas::reproject_poly(
        &rissgebiet,
        &target_proj,
        &latlon_proj,
        nas::UseRadians::None,
        false,
    )
}

#[wasm_bindgen]
pub fn get_problem_geojson() -> String {
    let proj = "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33";

    let poly_string1: &str = include_str!("./test1.txt");
    let poly_string2: &str = "";

    let s1 = serde_json::from_str::<SvgPolygonInner>(&poly_string1.trim()).unwrap_or_default();
    let s2 = serde_json::from_str::<Vec<SvgPolygonInner>>(&poly_string1.trim()).unwrap_or_default();
    let joined = s2; // crate::ops::join_polys(&s2);

    let s1 = crate::pdf::reproject_poly_back_into_latlon(&s1, proj).unwrap_or_default();
    let s2 = joined.iter().filter_map(|q| crate::pdf::reproject_poly_back_into_latlon(&q, proj).ok()).collect::<Vec<_>>();

    let v1 = vec![TaggedPolygon {
        poly: s1.clone(),
        attributes: BTreeMap::new(),
    }];

    let v2 = s2.iter().map(|e| {
        TaggedPolygon {
            poly: e.clone(),
            attributes: BTreeMap::new(),
        }
    }).collect::<Vec<_>>();

    
    serde_json::to_string(&GeoJSONResult {
        geojson1: crate::nas::tagged_polys_to_featurecollection(&v1),
        geojson2: crate::nas::tagged_polys_to_featurecollection(&v2),
        bounds: s2.first().map(|q| q.get_fit_bounds()).unwrap_or(s1.get_fit_bounds()),
    })
    .unwrap_or_default()
}

#[derive(Debug, Serialize, Deserialize)]
struct GetCoordsReturn {
    coords: [LatLng; 2],
    projection: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFlstInternalReturn {
    id_input: String,
    id_nice: String,
    ebene: String,
    flst: SvgPolygonInner,
}

#[wasm_bindgen]
pub fn search_flst_internal(id: String, nas_projected: Option<String>) -> Option<String> {
    let id_target = FlstIdParsed::from_str(&id).parse_num()?.format_nice();
    let nas =
        serde_json::from_str::<NasXMLFile>(&nas_projected.unwrap_or_default()).unwrap_or_default();
    nas.ebenen.get("AX_Flurstueck").and_then(|s| {
        s.iter().find_map(|s| {
            let found_id = FlstIdParsed::from_str(
                &s.attributes
                    .get("AX_Flurstueck")
                    .or_else(|| s.attributes.get("flurstueckskennzeichen"))?,
            )
            .parse_num()?
            .format_nice();

            if found_id == id_target {
                Some(
                    serde_json::to_string(&SearchFlstInternalReturn {
                        id_input: id.clone(),
                        id_nice: id_target.clone(),
                        ebene: "AX_Flurstueck".to_string(),
                        flst: s.poly.clone(),
                    })
                    .ok()?,
                )
            } else {
                None
            }
        })
    })
}

#[wasm_bindgen]
pub fn search_flst_part_internal(
    id: String,
    split_nas_projected: Option<String>,
) -> Option<String> {
    let nas = serde_json::from_str::<SplitNasXml>(&split_nas_projected.unwrap_or_default())
        .unwrap_or_default();
    nas.flurstuecke_nutzungen.iter().find_map(|(k, v)| {
        let id_nice = FlstIdParsed::from_str(&k).parse_num()?.format_nice();
        v.iter().find_map(|s| {
            let part_id = s.get_flst_part_id()?;
            if part_id == id {
                Some(
                    serde_json::to_string(&SearchFlstInternalReturn {
                        id_input: id.clone(),
                        id_nice: id_nice.clone(),
                        ebene: s.get_ebene().unwrap_or_default(),
                        flst: s.poly.clone(),
                    })
                    .ok()?,
                )
            } else {
                None
            }
        })
    })
}

#[wasm_bindgen]
pub fn validate_format_flst_id(id: String) -> String {
    FlstIdParsed::from_str(&id)
        .parse_num()
        .map(|s| s.format_nice())
        .unwrap_or_default()
}

#[wasm_bindgen]
pub async fn export_pdf_overview(
    konfiguration: Option<String>,
    nas_original: Option<String>,
    split_nas_xml: Option<String>,
    aenderungen: Option<String>,
    csv: Option<String>,
    use_dgm: bool,
    use_background: bool,
) -> Vec<u8> {
    let split_nas_xml =
        match serde_json::from_str::<SplitNasXml>(&split_nas_xml.unwrap_or_default()) {
            Ok(s) => s,
            Err(e) => {
                log_status("Error PDF overview parse split_nas_xml");
                log_status(&e.to_string());
                SplitNasXml::default()
            }
        };
    let nas_original = match serde_json::from_str::<NasXMLFile>(&nas_original.unwrap_or_default()) {
        Ok(s) => s,
        Err(e) => {
            log_status("Error PDF overview parse nas_original");
            log_status(&e.to_string());
            NasXMLFile::default()
        }
    };
    let konfiguration =
        match serde_json::from_str::<Konfiguration>(&konfiguration.unwrap_or_default()) {
            Ok(s) => s,
            Err(e) => {
                log_status("Error PDF overview parse konfiguration");
                log_status(&e.to_string());
                Konfiguration::default()
            }
        };
    let csv_data = match serde_json::from_str::<CsvDataType>(&csv.unwrap_or_default()) {
        Ok(s) => s,
        Err(e) => {
            log_status("Error PDF overview parse csv_data");
            log_status(&e.to_string());
            CsvDataType::default()
        }
    };
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen.unwrap_or_default()).unwrap_or_default();
    let aenderungen = reproject_aenderungen_back_into_latlon(&aenderungen, &split_nas_xml.crs).unwrap_or_default();
    let nas_migrated = nas_original.fortfuehren(&aenderungen, &split_nas_xml);
    let split_nas = if aenderungen != Aenderungen::default() {
        crate::nas::split_xml_flurstuecke_inner(&nas_migrated, &mut Vec::new()).unwrap_or(split_nas_xml)
    } else {
        split_nas_xml
    };

    log_status("ok overview exporting...");
    crate::pdf::export_overview(
        &konfiguration,
        &nas_migrated,
        &split_nas,
        &csv_data,
        use_dgm,
        use_background,
    )
    .await
}

#[wasm_bindgen]
pub fn get_header_coords(rc: String, utm_crs: Option<String>) -> String {
    let utm_crs =
        utm_crs.unwrap_or_else(|| "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33".to_string());

    let rc = match serde_json::from_str::<RissConfig>(rc.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let header_width_mm = 175.0;
    let header_height_mm = 35.0;
    let header_width_m = header_width_mm * rc.scale as f64 / 1000.0;
    let header_height_m = header_height_mm * rc.scale as f64 / 1000.0;

    let extent = match rc.get_extent_special(&utm_crs) {
        Some(o) => o,
        None => return "error1".to_string(),
    };

    let extent = match extent.reproject(&utm_crs) {
        Some(o) => o,
        None => return "error2".to_string(),
    };

    let a = match reproject_point_back_into_latlon(
        &SvgPoint {
            x: extent.min_x,
            y: extent.max_y,
        },
        &utm_crs,
    ) {
        Ok(o) => o,
        Err(e) => return e,
    };

    let b = match reproject_point_back_into_latlon(
        &SvgPoint {
            x: extent.min_x + header_width_m,
            y: extent.max_y - header_height_m,
        },
        &utm_crs,
    ) {
        Ok(o) => o,
        Err(e) => return e,
    };

    serde_json::to_string(&GetCoordsReturn {
        coords: [LatLng { lat: a.y, lng: a.x }, LatLng { lat: b.y, lng: b.x }],
        projection: "utm".to_string(),
    })
    .unwrap_or_default()
}

#[wasm_bindgen]
pub fn lock_unlock_poly(id: Option<String>, aenderungen: String) -> String {
    let id = id.and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.trim().to_string())
        }
    });

    let mut aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let ids_to_lock = match id {
        Some(s) => vec![s],
        None => aenderungen
            .na_polygone_neu
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
    };

    let mut l = Vec::new();
    for i in ids_to_lock {
        if let Some(sm) = aenderungen.na_polygone_neu.get_mut(&i) {
            sm.locked = !sm.locked;
            l.push(format!("ok locking aenderung {i}"));
        }
    }

    serde_json::to_string(&CleanStageResult {
        aenderungen: aenderungen,
        log: l,
    })
    .unwrap_or_default()
}

#[wasm_bindgen]
pub fn lib_nutzungen_saeubern(
    id: Option<String>,
    aenderungen: String,
    split_nas_xml: String,
    nas_original: String,
    konfiguration: String,
) -> String {
    let id = id.and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.trim().to_string())
        }
    });

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

    let aenderungen =
        match reproject_aenderungen_into_target_space(&aenderungen, &split_nas_xml.crs) {
            Ok(o) => o,
            Err(e) => return e.to_string(),
        };

    let mut log = Vec::new();

    log.push(format!(
        "cleaning {} aenderungen, id poly = {id:?}",
        aenderungen.na_polygone_neu.len()
    ));

    let clean = aenderungen
        .clean_stage1(
            konfiguration.merge.stage1_maxdst_point,
            konfiguration.merge.stage1_maxdst_line,
            false,
        )
        .clean_stage2(1.0, 1.0, 10.0, false)
        .clean_stage3(
            &split_nas_xml,
            &mut log,
            konfiguration.merge.stage2_maxdst_point,
            konfiguration.merge.stage2_maxdst_line,
            false,
        )
        .clean_stage4(
            &nas_original,
            &mut log,
            konfiguration.merge.stage3_maxdst_line,
            konfiguration.merge.stage3_maxdst_line2,
            konfiguration.merge.stage3_maxdeviation_followline,
            false
        );

    log.push(format!(
        "cleaned {} aenderungen!",
        aenderungen.na_polygone_neu.len()
    ));

    let clean = match id {
        Some(s) => match clean.na_polygone_neu.get(&s) {
            Some(q) => {
                let mut aenderungen_clone = aenderungen.clone();
                aenderungen_clone
                    .na_polygone_neu
                    .insert(s.clone(), q.clone());
                aenderungen_clone
            }
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
    })
    .unwrap_or_default()
}

#[wasm_bindgen]
pub fn lib_get_aenderungen_clean(
    id: Option<String>,
    aenderungen: Option<String>,
    split_nas_xml: Option<String>,
    nas_original: Option<String>,
    konfiguration: Option<String>,
    csv: Option<String>,
) -> String {
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen.unwrap_or_default()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let split_nas_xml =
        serde_json::from_str::<SplitNasXml>(&split_nas_xml.unwrap_or_default()).unwrap_or_default();
    let nas_original =
        serde_json::from_str::<NasXMLFile>(&nas_original.unwrap_or_default()).unwrap_or_default();
    let konfiguration = serde_json::from_str::<Konfiguration>(&konfiguration.unwrap_or_default())
        .unwrap_or_default();
    let csv_data =
        serde_json::from_str::<CsvDataType>(&csv.unwrap_or_default()).unwrap_or_default();

    let aenderungen =
        match reproject_aenderungen_into_target_space(&aenderungen, &split_nas_xml.crs) {
            Ok(o) => o,
            Err(e) => return e.to_string(),
        };

    let mut log = Vec::new();
    let id = id.unwrap_or_default();

    log.push(format!(
        "cleaning {} aenderungen, stage = {id}",
        aenderungen.na_polygone_neu.len()
    ));

    let force = false;
    let clean = match id.as_str() {
        "1" => aenderungen.clean_stage1(
            konfiguration.merge.stage1_maxdst_point,
            konfiguration.merge.stage1_maxdst_line,
            force,
        ),
        "2" => aenderungen.clean_stage2(1.0, 1.0, 10.0, force),
        "25" => aenderungen.clean_stage25(force),
        "3" => aenderungen.clean_stage3(
            &split_nas_xml,
            &mut log,
            konfiguration.merge.stage2_maxdst_point,
            konfiguration.merge.stage2_maxdst_line,
            force,
        ),
        "4" => aenderungen.clean_stage4(
            &nas_original,
            &mut log,
            konfiguration.merge.stage3_maxdst_line,
            konfiguration.merge.stage3_maxdst_line2,
            konfiguration.merge.stage3_maxdeviation_followline,
            force,
        ),
        "13" => aenderungen
            .clean_stage1(
                konfiguration.merge.stage1_maxdst_point,
                konfiguration.merge.stage1_maxdst_line,
                force,
            )
            .clean_stage2(1.0, 1.0, 10.0, force)
            .clean_stage3(
                &split_nas_xml,
                &mut log,
                konfiguration.merge.stage2_maxdst_point,
                konfiguration.merge.stage2_maxdst_line,
                force,
            )
            .clean_stage4(
                &nas_original,
                &mut log,
                konfiguration.merge.stage3_maxdst_line,
                konfiguration.merge.stage3_maxdst_line2,
                konfiguration.merge.stage3_maxdeviation_followline,
                force,
            ),
        "5" => aenderungen.clean_stage5(&split_nas_xml, &mut log, force),
        "7" => aenderungen.show_splitflaechen(&split_nas_xml, &nas_original, &csv_data),
        "8" => aenderungen.zu_david(&nas_original, &split_nas_xml),
        _ => return format!("wrong id {id}"),
    };

    log.push(format!(
        "cleaned {} aenderungen!",
        aenderungen.na_polygone_neu.len()
    ));

    let clean = match reproject_aenderungen_back_into_latlon(&clean, &split_nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    serde_json::to_string(&CleanStageResult {
        aenderungen: clean,
        log,
    })
    .unwrap_or_default()
}

#[wasm_bindgen]
pub async fn aenderungen_zu_geograf(
    split_nas_xml: String,
    nas_xml: String,
    projekt_info: String,
    konfiguration: String,
    aenderungen: String,
    risse: String,
    csv_data: String,
    render_hintergrund_vorschau: bool,
    use_dgm: bool,
) -> Vec<u8> {
    log_status_clear();
    log_status("Starte Export nach GEOgraf...");

    let split_nas_xml =
        serde_json::from_str::<SplitNasXml>(split_nas_xml.as_str()).unwrap_or_default();
    let nas_xml = serde_json::from_str::<NasXMLFile>(nas_xml.as_str()).unwrap_or_default();
    let projekt_info =
        serde_json::from_str::<ProjektInfo>(projekt_info.as_str()).unwrap_or_default();
    let konfiguration =
        serde_json::from_str::<Konfiguration>(konfiguration.as_str()).unwrap_or_default();
    let aenderungen = serde_json::from_str::<Aenderungen>(aenderungen.as_str()).unwrap_or_default();
    let risse = serde_json::from_str::<Risse>(&risse).unwrap_or_default();
    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or_default();

    crate::geograf::export_aenderungen_geograf(
        &split_nas_xml,
        &nas_xml,
        &projekt_info,
        &konfiguration,
        &aenderungen,
        &risse,
        &csv_data,
        render_hintergrund_vorschau,
        use_dgm,
    )
    .await
}

#[wasm_bindgen]
pub fn aenderungen_zu_nas_xml(
    aenderungen: String,
    nas_xml: String,
    split_nas: String,
    xml_objects: String,
) -> String {
    let aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let nas_xml = match serde_json::from_str::<NasXMLFile>(&nas_xml) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let split_nas = match serde_json::from_str::<SplitNasXml>(&split_nas) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let xml_objects = match serde_json::from_str::<NasXmlObjects>(&xml_objects) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    crate::david::aenderungen_zu_nas_xml(&aenderungen, &nas_xml, &split_nas, &xml_objects)
}

#[wasm_bindgen]
pub fn aenderungen_zu_david(
    datum: String,
    aenderungen: String,
    nas_xml: String,
    split_nas: String,
    xml_objects: String,
) -> String {
    let datum = match chrono::DateTime::parse_from_rfc3339(&datum) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let nas_xml = match serde_json::from_str::<NasXMLFile>(&nas_xml) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let split_nas = match serde_json::from_str::<SplitNasXml>(&split_nas) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &nas_xml.crs) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let xml_objects = match serde_json::from_str::<NasXmlObjects>(&xml_objects) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    crate::david::aenderungen_zu_fa_xml(&aenderungen, &nas_xml, &split_nas, &xml_objects, &datum)
}

#[wasm_bindgen]
pub fn get_geojson_fuer_neue_polygone(aenderungen: String, target_crs: String) -> String {
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct NeuePolygoneGeoJson {
        nutzung_definiert: bool,
        geojson: String,
    }

    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();

    let construct_polys = |(k, v): (&String, &PolyNeu)| TaggedPolygon {
        attributes: vec![("newPolyId".to_string(), k.clone())]
            .into_iter()
            .collect(),
        poly: reproject_poly_back_into_latlon(v.poly.get_inner(), &target_crs),
    };

    let nutzung_definiert = aenderungen
        .na_polygone_neu
        .iter()
        .filter(|(_, poly)| poly.nutzung.is_some())
        .map(construct_polys)
        .collect::<Vec<_>>();

    let nutzung_definiert = NeuePolygoneGeoJson {
        nutzung_definiert: true,
        geojson: crate::nas::tagged_polys_to_featurecollection(&nutzung_definiert),
    };

    let nutzung_nicht_definiert = aenderungen
        .na_polygone_neu
        .iter()
        .filter(|(_, poly)| poly.nutzung.is_none())
        .map(construct_polys)
        .collect::<Vec<_>>();

    let nutzung_nicht_definiert = NeuePolygoneGeoJson {
        nutzung_definiert: false,
        geojson: crate::nas::tagged_polys_to_featurecollection(&nutzung_nicht_definiert),
    };

    serde_json::to_string(&[nutzung_definiert, nutzung_nicht_definiert]).unwrap_or_default()
}

#[wasm_bindgen]
pub fn get_polyline_guides_in_current_bounds(
    split_flurstuecke: String,
    crs: Option<String>,
    aenderungen: String,
    map_bounds: String,
) -> String {
    #[allow(non_snake_case)]
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
    struct MapBounds {
        _northEast: crate::LatLng,
        _southWest: crate::LatLng,
    }

    let crs = crs.unwrap_or_else(|| default_etrs33());
    let mut pl = Vec::new();
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();
    let aenderungen = reproject_aenderungen_back_into_latlon(&aenderungen, &crs).unwrap_or(aenderungen);
    match serde_json::from_str::<MapBounds>(&map_bounds) {
        Ok(MapBounds {
            _northEast,
            _southWest,
        }) => {
            let rect = quadtree_f32::Rect {
                min_x: _southWest.lng,
                min_y: _southWest.lat,
                max_x: _northEast.lng,
                max_y: _northEast.lat,
            };
            let mut ringe = split_fs.get_polyline_guides_in_bounds(rect);
            let mut aenderungen_ringe = aenderungen
                .na_polygone_neu
                .values()
                .flat_map(|p| {
                    let mut p_inner = p.poly.get_inner();
                    let mut v = vec![p_inner.outer_ring];
                    v.append(&mut p_inner.inner_rings);
                    v.into_iter()
                })
                .collect::<Vec<_>>();
            ringe.append(&mut aenderungen_ringe);
            pl = ringe
                .iter()
                .map(|svg_line| {
                    svg_line
                        .points
                        .iter()
                        .map(|p| [p.y, p.x])
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
        }
        Err(_e) => {}
    }
    serde_json::to_string(&pl).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

#[wasm_bindgen]
pub fn fixup_polyline_rissgebiet(points: String, crs: String) -> String {
    let points = serde_json::from_str::<Vec<LatLng>>(&points).unwrap_or_default();
    match fixup_polyline_internal(&points).map(|s| project_poly_into_target_crs(s, &crs)) {
        Some(s) => serde_json::to_string(&s).unwrap_or_default(),
        None => "invalid LatLng".to_string(),
    }
}

fn project_poly_into_target_crs(rissgebiet: SvgPolygonInner, crs: &str) -> SvgPolygonInner {

    let already_reprojected = rissgebiet.outer_ring.points.iter().any(|s| s.x > 1000.0 || s.y > 1000.0);
    
    if already_reprojected {
        return rissgebiet;
    }
    
    let latlon_proj = match proj4rs::Proj::from_proj_string(crate::nas::LATLON_STRING) {
        Ok(o) => o,
        Err(_) => return rissgebiet,
    };

    let target_proj = match proj4rs::Proj::from_proj_string(&crs) {
        Ok(o) => o,
        Err(_) => return rissgebiet,
    };

    crate::nas::reproject_poly(
        &rissgebiet,
        &latlon_proj,
        &target_proj,
        nas::UseRadians::ForSourceAndTarget,
        true,
    )
}

#[wasm_bindgen]
pub fn reproject_aenderungen_for_view(aenderungen: String, target_crs: String) -> String {
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();
    match reproject_aenderungen_into_target_space(&aenderungen, &target_crs) {
        Ok(o) => serde_json::to_string(&o).unwrap_or_default(),
        Err(_) => serde_json::to_string(&aenderungen).unwrap_or_default(),
    }
}

#[wasm_bindgen]
pub fn fixup_polyline(
    xml: String, 
    split_flurstuecke: String, 
    points: String, 
    id: String, 
    aenderungen: String, 
    config: String
) -> String {
    use crate::nas::SplitNasXml;

    let nas_xml = serde_json::from_str::<NasXMLFile>(&xml).unwrap_or_default();
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke).unwrap_or_default();
    let points = serde_json::from_str::<Vec<LatLng>>(&points).unwrap_or_default();
    let aenderungen = serde_json::from_str::<Aenderungen>(&aenderungen).unwrap_or_default();
    let konfiguration = serde_json::from_str::<Konfiguration>(&config).unwrap_or_default();
    
    let force = false;
    if let Some(s) = fixup_polyline_internal(&points).map(|s| project_poly_into_target_crs(s, &split_fs.crs)) {
        
        let mut aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &split_fs.crs) {
            Ok(o) => o,
            Err(_) => return serde_json::to_string(&aenderungen).unwrap_or_default(),
        };

        log_status(&format!("new poly {s:?}"));
        log_status(&format!("aenderungen initial {:?}", aenderungen.na_polygone_neu));

        aenderungen.na_polygone_neu.insert(id, crate::ui::PolyNeu {
            poly: SvgPolygon::Old(s),
            nutzung: None,
            locked: false,
        });

        log_status(&format!("aenderungen inserted {:?}", aenderungen.na_polygone_neu));

        let aenderungen = aenderungen.clean_stage1(
            konfiguration.merge.stage1_maxdst_point,
            konfiguration.merge.stage1_maxdst_line,
            force,
        );

        let aenderungen = aenderungen.clean_stage2(1.0, 1.0, 10.0, force);

        let aenderungen = aenderungen.clean_stage3(
            &split_fs,
            &mut Vec::new(),
            konfiguration.merge.stage2_maxdst_point,
            konfiguration.merge.stage2_maxdst_line,
            force,
        );

        let aenderungen = aenderungen.clean_stage4(
            &nas_xml,
            &mut Vec::new(),
            konfiguration.merge.stage3_maxdst_line,
            konfiguration.merge.stage3_maxdst_line2,
            konfiguration.merge.stage3_maxdeviation_followline,
            force,
        );

        let aenderungen = aenderungen.deduplicate(force);

        serde_json::to_string(&aenderungen).unwrap_or_default()
    } else {
        serde_json::to_string(&aenderungen).unwrap_or_default()
    }
}


fn fixup_polyline_internal(
    points: &[LatLng],
) -> Option<SvgPolygonInner> {
    let mut points = points.to_vec();

    if points.first()? != points.last()? {
        points.push(points.first()?.clone());
    }

    Some(SvgPolygonInner {
        outer_ring: SvgLine {
            points: points
                .iter()
                .map(|p| SvgPoint { x: p.lng, y: p.lat })
                .collect(),
        },
        inner_rings: Vec::new(),
    })
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
    crate::ui::render_entire_screen(
        &projektinfo,
        &risse,
        &uidata,
        &csv,
        &aenderungen,
        &konfiguration,
    )
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
pub fn ui_render_switch_content(uidata: String) -> String {
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
    split_flurstuecke: Option<String>,
) -> String {
    let projektinfo = serde_json::from_str::<ProjektInfo>(&projektinfo).unwrap_or_default();
    let risse = serde_json::from_str::<Risse>(&risse).unwrap_or_default();
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let uidata = UiData::from_string(&uidata);
    let csv_data = serde_json::from_str::<CsvDataType>(&csv_data).unwrap_or(CsvDataType::default());
    let split_fs = serde_json::from_str::<SplitNasXml>(&split_flurstuecke.unwrap_or_default())
        .unwrap_or_default();
    if uidata.secondary_content.unwrap_or_default() {
        crate::ui::render_secondary_content(&aenderungen)
    } else {
        crate::ui::render_project_content(
            &projektinfo,
            &risse,
            &csv_data,
            &aenderungen,
            &uidata,
            &split_fs,
        )
    }
}

#[wasm_bindgen]
pub fn get_geojson_polygon(s: String) -> String {
    let flst = match serde_json::from_str::<SvgPolygonInner>(&s) {
        Ok(o) => TaggedPolygon {
            poly: o,
            attributes: BTreeMap::new(),
        },
        Err(e) => return e.to_string(),
    };
    crate::nas::tagged_polys_to_featurecollection(&[flst])
}

#[wasm_bindgen]
pub fn get_fit_bounds(s: String) -> String {
    let flst = match serde_json::from_str::<SvgPolygon>(&s) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    }.get_inner();
    let crs = "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33";
    let flst = reproject_poly_back_into_latlon(flst, crs);
    let bounds = flst.get_fit_bounds();
    serde_json::to_string(&bounds).unwrap_or_default()
}

#[wasm_bindgen]
pub fn search_for_polyneu(aenderungen: String, poly_id: String) -> String {
    let aenderungen = match serde_json::from_str::<Aenderungen>(&aenderungen) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };

    let tp = aenderungen.na_polygone_neu.iter().find_map(|(k, v)| {
        if k.as_str() == poly_id.as_str() {
            Some(v.poly.clone())
        } else {
            None
        }
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
    let r = ax_gebauede
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
    pub xml_objects: NasXmlObjects,
    pub nas_original: NasXMLFile,
    pub nas_cut_original: SplitNasXml,
    pub nas_projected: NasXMLFile,
    pub nas_cut_projected: SplitNasXml,
}

#[wasm_bindgen]
pub fn get_ebenen_darstellung(konfiguration: String) -> String {
    let konfiguration = match serde_json::from_str::<Konfiguration>(&konfiguration) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    let arr = konfiguration
        .style
        .get_styles_sorted()
        .iter()
        .map(|(_k, v)| v.name.clone())
        .collect::<Vec<_>>();
    serde_json::to_string(&arr).unwrap_or_default()
}

#[derive(Debug, Serialize, Deserialize)]
struct NasParseError {
    error: String,
    log: Vec<String>,
}

#[wasm_bindgen]
pub fn load_nas_xml(s: String, style: String) -> String {
    log_status("load_nas_xml 1");
    let konfiguration = match serde_json::from_str::<Konfiguration>(&style) {
        Ok(o) => o,
        Err(e) => return e.to_string(),
    };
    log_status("konfiguration ok");
    let mut t = crate::get_nutzungsartenkatalog_ebenen().values().cloned().collect::<BTreeSet<_>>();
    t.insert("AX_BauRaumOderBodenordnungsrecht".to_string());
    t.insert("AX_Flurstueck".to_string());
    t.insert("AX_Gebaeude".to_string());
    t.insert("AX_Bauteil".to_string());
    t.insert("AX_BauwerkImVerkehrsbereich".to_string());
    t.insert("AX_SonstigesBauwerkOderSonstigeEinrichtung".to_string());
    t.insert("AX_BauwerkOderAnlageFuerIndustrieUndGewerbe".to_string());

    let mut log = Vec::new();
    log_status(&format!("parsing XML: types = {t:?}"));

    let (xml_parsed, xml_objects, nas_original) = match serde_json::from_str::<NasXMLFile>(&s) {
        Ok(o) => (Vec::new(), NasXmlObjects::default(), o),
        Err(_) => {
            let xml_parsed = match crate::xml::parse_xml_string(&s, &mut log) {
                Ok(o) => o,
                Err(e) => {
                    return serde_json::to_string(&NasParseError {
                        error: format!("XML parse error: {e:?}"),
                        log: log,
                    })
                    .unwrap_or_default()
                }
            };
            log_status("xml parsed");
            let xml_objects = crate::nas::parse_nas_xml_objects(&xml_parsed);
            log_status("xml objects parsed");
            let nas_original = match crate::nas::parse_nas_xml(xml_parsed.clone(), &t) {
                Ok(o) => o,
                Err(e) => {
                    return serde_json::to_string(&NasParseError { error: e, log: log }).unwrap_or_default()
                }
            };
            (xml_parsed, xml_objects, nas_original)
        }
    };

    log_status("nas original ok");
    let nas_cut_original = match crate::nas::split_xml_flurstuecke_inner(&nas_original, &mut log) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&NasParseError { error: e, log: log }).unwrap_or_default()
        }
    };
    log_status("nas cut ok");
    let nas_projected = match crate::nas::transform_nas_xml_to_lat_lon(&nas_original, &mut log) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&NasParseError { error: e, log: log }).unwrap_or_default()
        }
    };
    log_status("nas projected ok");
    let mut nas_cut_projected =
        match crate::nas::transform_split_nas_xml_to_lat_lon(&nas_cut_original, &mut log) {
            Ok(o) => o,
            Err(e) => {
                return serde_json::to_string(&NasParseError { error: e, log: log })
                    .unwrap_or_default()
            }
        };
    log_status("nas cut projected ok");
    crate::nas::fixup_flst_groesse(&nas_cut_original, &mut nas_cut_projected);
    log_status("NAS XML ok!");
    serde_json::to_string(&LoadNasReturn {
        log,
        xml_parsed,
        nas_original,
        nas_cut_original,
        nas_projected,
        nas_cut_projected,
        xml_objects,
    })
    .unwrap_or_default()
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
    let ls = konfiguration
        .style
        .ebenen
        .iter()
        .find(|(_, s)| s.name.trim() == layer_name.trim())
        .map(|(_, v)| v.clone());
    let ls = match ls {
        Some(s) => s,
        None => return format!("style für {layer_name} nicht gefunden"),
    };
    serde_json::to_string(&ls).unwrap_or_default()
}

#[wasm_bindgen]
pub fn get_gebaeude_geojson_fuer_aktive_flst(
    json: String,
    csv: String,
    aenderungen: String,
) -> String {
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
    ignore_firstline: String,
) -> String {
    let csv = decode(csv);
    let csv_daten = match crate::csv::parse_csv(
        &csv,
        &id_col,
        &nutzung_col,
        &eigentuemer_col,
        &delimiter,
        ignore_firstline == "true",
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
        .collect::<BTreeSet<_>>();

    let nas_parsed_complete = match parse_nas_xml(nas_projected, &kuerzel) {
        Ok(s) => s,
        Err(_) => NasXMLFile::default(),
    };

    let tp_count = nas_parsed_complete
        .ebenen
        .iter()
        .map(|(k, s)| (k.clone(), s.len()))
        .collect::<BTreeMap<_, _>>();

    log.push(format!("tp_count: {:?}", tp_count));

    let alle_auto_kuerzel = nas_parsed_complete
        .ebenen
        .iter()
        .flat_map(|(_, s)| s.into_iter().filter_map(|tp| tp.get_auto_kuerzel()))
        .collect::<BTreeSet<_>>();

    let neue_ebenen = alle_auto_kuerzel
        .into_iter()
        .map(|ak| {
            (get_new_poly_id(), {
                let mut m = PdfEbenenStyle::default();
                m.kuerzel = ak;
                m.fill_color = Some(crate::uuid_wasm::random_color());
                m
            })
        })
        .collect::<Vec<_>>();

    config.pdf.nutzungsarten = neue_ebenen.iter().cloned().collect();
    config.pdf.layer_ordnung = neue_ebenen.iter().map(|(k, _)| k.clone()).collect();
    serde_json::to_string(&KonfigurationLayerAlle {
        result: config,
        log,
    })
    .unwrap_or_default()
}

#[wasm_bindgen]
pub fn edit_konfiguration_layer_neu(konfiguration: String, layer_type: String) -> String {
    let mut config = serde_json::from_str::<Konfiguration>(&konfiguration).unwrap_or_default();
    match layer_type.as_str() {
        "style" => {
            let ebene_id = get_new_poly_id();
            config
                .style
                .ebenen
                .insert(ebene_id.clone(), EbenenStyle::default());
            let mut copy = config.style.ebenen_ordnung.clone();
            copy.reverse();
            copy.push(ebene_id);
            copy.reverse();
            config.style.ebenen_ordnung = copy;
        }
        "pdf-nutzungsarten" => {
            let ebene_id = get_new_poly_id();
            config
                .pdf
                .nutzungsarten
                .insert(ebene_id.clone(), PdfEbenenStyle::default());
            let mut copy = config.pdf.layer_ordnung.clone();
            copy.reverse();
            copy.push(ebene_id);
            copy.reverse();
            config.pdf.layer_ordnung = copy;
        }
        _ => {}
    }
    serde_json::to_string(&config).unwrap_or_default()
}

#[wasm_bindgen]
pub fn edit_konfiguration_move_layer(
    konfiguration: String,
    layer_type: String,
    ebene_id: String,
    move_type: String,
) -> String {
    let mut config = serde_json::from_str::<Konfiguration>(&konfiguration).unwrap_or_default();
    match move_type.as_str() {
        "delete" => match layer_type.as_str() {
            "style" => {
                config.style.ebenen.remove(ebene_id.as_str());
                config.style.ebenen_ordnung.retain(|s| *s != ebene_id);
            }
            "pdf-nutzungsarten" => {
                config.pdf.nutzungsarten.remove(ebene_id.as_str());
                config.pdf.layer_ordnung.retain(|s| *s != ebene_id);
            }
            _ => {}
        },
        "move-up" => match layer_type.as_str() {
            "style" => {
                let mut temp = ebene_id.clone();
                if let Some(pos) = config
                    .style
                    .ebenen_ordnung
                    .iter()
                    .position(|s| s.as_str() == ebene_id)
                {
                    if let Some(next) = config.style.ebenen_ordnung.get_mut(pos.saturating_sub(1)) {
                        std::mem::swap(&mut temp, next);
                    }
                    if let Some(pos_st) = config.style.ebenen_ordnung.get_mut(pos) {
                        std::mem::swap(&mut temp, pos_st);
                    }
                }
            }
            "pdf-nutzungsarten" => {
                let mut temp = ebene_id.clone();
                if let Some(pos) = config
                    .pdf
                    .layer_ordnung
                    .iter()
                    .position(|s| s.as_str() == ebene_id)
                {
                    if let Some(next) = config.pdf.layer_ordnung.get_mut(pos.saturating_sub(1)) {
                        std::mem::swap(&mut temp, next);
                    }
                    if let Some(pos_st) = config.pdf.layer_ordnung.get_mut(pos) {
                        std::mem::swap(&mut temp, pos_st);
                    }
                }
            }
            _ => {}
        },
        "move-down" => match layer_type.as_str() {
            "style" => {
                let mut temp = ebene_id.clone();
                if let Some(pos) = config
                    .style
                    .ebenen_ordnung
                    .iter()
                    .position(|s| s.as_str() == ebene_id)
                {
                    if let Some(next) = config.style.ebenen_ordnung.get_mut(pos.saturating_add(1)) {
                        std::mem::swap(&mut temp, next);
                    }
                    if let Some(pos_st) = config.style.ebenen_ordnung.get_mut(pos) {
                        std::mem::swap(&mut temp, pos_st);
                    }
                }
            }
            "pdf-nutzungsarten" => {
                let mut temp = ebene_id.clone();
                if let Some(pos) = config
                    .pdf
                    .layer_ordnung
                    .iter()
                    .position(|s| s.as_str() == ebene_id)
                {
                    if let Some(next) = config.pdf.layer_ordnung.get_mut(pos.saturating_add(1)) {
                        std::mem::swap(&mut temp, next);
                    }
                    if let Some(pos_st) = config.pdf.layer_ordnung.get_mut(pos) {
                        std::mem::swap(&mut temp, pos_st);
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    serde_json::to_string(&config).unwrap_or_default()
}

pub fn get_nutzungsartenkatalog() -> BTreeMap<String, crate::search::NutzungsArt> {
    crate::search::get_nutzungsartenkatalog()
}

pub fn get_nutzungsartenkatalog_ebenen() -> BTreeMap<String, String> {
    get_nutzungsartenkatalog()
        .iter()
        .filter_map(|(k, v)| {
            Some((
                k.clone(),
                v.atr.split(",").find_map(|s| {
                    let mut sp = s.split("=");
                    let k = sp.next()?;
                    let v = sp.next()?;
                    if k == "AX_Ebene" {
                        Some(v.to_string())
                    } else {
                        None
                    }
                })?,
            ))
        })
        .collect()
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
