use crate::{
    csv::CsvDataType, nas::{
        self, MemberObject, NasXMLFile, NasXmlObjects, NasXmlQuadTree, SplitNasXml, SplitNasXmlQuadTree, SvgLine, SvgPoint, SvgPolygon, SvgPolygonInner, TaggedPolygon
    }, ops::{
        intersect_polys, join_polys, join_polys_fast, subtract_from_poly
    }, ui::{Aenderungen, PolyNeu}, uuid_wasm::{
        log_status, log_status_clear, uuid
    }
};
use std::collections::{
    BTreeMap,
    BTreeSet,
};

pub struct AenderungObject {
    orig_change_id: String,
    neu_kuerzel: String,
    neu_ebene: String,
    poly: TaggedPolygon,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Operation {
    Delete {
        obj_id: String,
        ebene: String,
        kuerzel: String,
        poly_alt: SvgPolygonInner,
    },
    Replace {
        obj_id: String,
        ebene: String,
        kuerzel: String,
        poly_alt: SvgPolygonInner,
        poly_neu: SvgPolygonInner,
    },
    Insert {
        ebene: String,
        kuerzel: String,
        poly_neu: SvgPolygonInner,
    },
}

impl Operation {
    fn get_str_id(&self) -> String {
        match self {
            Operation::Delete { obj_id, ebene, kuerzel , poly_alt } => format!("Delete:{obj_id}::{ebene}::{kuerzel}::{}", poly_alt.get_hash()),
            Operation::Replace { obj_id, ebene, kuerzel, poly_alt, poly_neu } => format!("Replace:{obj_id}::{ebene}::{kuerzel}::{}::{}", poly_alt.get_hash(), poly_neu.get_hash()),
            Operation::Insert { ebene, kuerzel, poly_neu } => format!("Insert:{ebene}::{kuerzel}::{}", poly_neu.get_hash()),
        }
    }
    fn get_geom_kuerzel_pair(&self) -> (u64, String) {
        match self {
            Operation::Delete { kuerzel , poly_alt, .. } => (poly_alt.get_hash(), kuerzel.clone()),
            Operation::Replace { kuerzel, poly_neu, .. } => (poly_neu.get_hash(), kuerzel.clone()),
            Operation::Insert {kuerzel, poly_neu, .. } => (poly_neu.get_hash(), kuerzel.clone()),
        }
    }
}

pub fn aenderungen_zu_fa_xml(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
    csv: &CsvDataType,
    objects: &NasXmlObjects,
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>,
) -> String {
    let aenderungen_gesamt = build_operations(aenderungen, nas_xml, split_nas, csv);
    crate::david::insert_gebaeude_delete(&aenderungen, &aenderungen_gesamt);
    operations_to_xml_file(&aenderungen_gesamt, objects, datum_jetzt)
}

pub fn build_operations(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
    csv: &CsvDataType,
) -> Vec<Operation> {

    log_status("joining gemarkung...");
    let fluren = nas_xml.get_fluren(csv);
    log_status(&format!("fluren len {}", fluren.len()));
    for f in fluren.iter() {
        log_status(&format!("flur: {} m2", f.area_m2().round()));
    }
    log_status("Gemarkung joined!");

    let fluren = subtract_bauraum_bodenordnung(&fluren, &nas_xml);

    let aenderungen_1 = crate::david::get_na_definiert_as_na_polyneu(aenderungen, split_nas, &fluren);
    let rm = crate::david::napoly_to_reverse_map(&aenderungen_1.na_polygone_neu, &nas_xml);
    let aenderungen_todo_1 = crate::david::reverse_map_to_aenderungen(&rm, false);
    // log_status("merge_and_intersect_inserts...");
    // let aenderungen_todo_1 = crate::david::merge_aenderungen_with_existing_nas(&aenderungen_todo_1, self, false);
    log_status("fortfuehren_internal...");
    let fortgefuehrt_1 = nas_xml.fortfuehren_internal(&aenderungen_todo_1); // okay bis hier

    log_status("NasXMLFile::fortfuehren");
    log_aenderungen(&aenderungen_todo_1);
    log_status("----");

    let aenderungen_2 = crate::david::get_aenderungen_prepared(aenderungen, &fortgefuehrt_1, split_nas, &fluren);
    let rm = crate::david::napoly_to_reverse_map(&aenderungen_2.na_polygone_neu, &fortgefuehrt_1);
    let aenderungen_todo_2 = crate::david::reverse_map_to_aenderungen(&rm, true);
    // let aenderungen_todo_2 = crate::david::merge_aenderungen_with_existing_nas(&aenderungen_todo_2, &fortgefuehrt_1, true);
    // let fortgefuehrt_2 = fortgefuehrt_1.fortfuehren_internal(&aenderungen_todo_2);

    log_status("NasXMLFile::fortfuehren");
    log_aenderungen(&aenderungen_todo_2);
    log_status("----");

    let mut aenderungen_gesamt = Vec::new();
    aenderungen_gesamt.extend(aenderungen_todo_1.iter().cloned());
    aenderungen_gesamt.extend(aenderungen_todo_2.iter().cloned());
    
    let mut aenderungen_3 = Aenderungen::default();
    aenderungen_3.na_polygone_neu.append(&mut aenderungen_1.na_polygone_neu.clone());
    aenderungen_3.na_polygone_neu.append(&mut aenderungen_2.na_polygone_neu.clone());

    log_status("merging inserts...");
    let aenderungen_gesamt = crate::david::merge_and_intersect_inserts(
        &aenderungen_gesamt,
        &aenderungen_3.na_polygone_neu,
    );
    log_status("inserts merged!");

    // let aenderungen_gesamt = filter_identic_operations(&aenderungen_gesamt);

    aenderungen_gesamt
}

pub fn subtract_bauraum_bodenordnung(
    fluren: &Vec<SvgPolygonInner>,
    nas_xml: &NasXMLFile
) -> Vec<SvgPolygonInner> {

    let d = Vec::new();
    let bauraum_bodenordnung_flst = nas_xml.ebenen
    .get("AX_BauRaumOderBodenordnungsrecht")
    .unwrap_or(&d)
    .iter()
    .map(|q| &q.poly)
    .collect::<Vec<_>>();

    if bauraum_bodenordnung_flst.is_empty() {
        return fluren.clone();
    }

    fluren.iter().flat_map(|s| {
        subtract_from_poly(s, &bauraum_bodenordnung_flst, false)
    }).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OpPrereq {
    Insert { id: String },
    Replace { id: String },
    Delete { id: String }
}

impl OpPrereq {
    pub fn get_id(&self) -> String {
        match self {
            OpPrereq::Insert { id } => id.clone(),
            OpPrereq::Replace { id } => id.clone(),
            OpPrereq::Delete { id } => id.clone(),
        }
    }
}

pub fn operations_to_xml_file_internal(
    aenderungen_todo: &[Operation], 
    objects: &NasXmlObjects, 
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>
) -> Vec<(String, OpPrereq)> {
    let mut final_strings = aenderungen_todo.iter()
    .enumerate()
    .filter_map(|(i, s)| {
        match s {
        Operation::Delete { obj_id, .. } => {
            let o = objects.objects.get(obj_id)?;
            if o.poly.is_empty() {
                return None; // TODO: Delete non-polygon objects (attributes, AP_PTO, etc.)
            }
            let beginnt = o.beginnt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true).replace("-", "").replace(":", "");
            let rid = format!("{obj_id}{beginnt}");
            let typename = &o.member_type;
            Some((
                format!("            <wfs:Delete typeName=\"{typename}\"><fes:Filter><fes:ResourceId rid=\"{rid}\" /></fes:Filter></wfs:Delete>"),
                OpPrereq::Delete { id: obj_id.clone() }
            ))
        },
        Operation::Insert { ebene, kuerzel, poly_neu } => {
            let mut auto_attribute = TaggedPolygon::get_auto_attributes_for_kuerzel(&kuerzel, &[]);
            auto_attribute.remove("AX_Ebene");
            let auto_attribute = auto_attribute.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>();
            let obj_id = "DE_001".to_string() + &format!("{i:010}");
            Some((get_insert_xml_node(
                    ebene,
                    &obj_id,
                    &auto_attribute,
                    datum_jetzt,
                    poly_neu,
                ),
                OpPrereq::Insert { id: obj_id.clone() }
            ))
        },
        Operation::Replace {
            obj_id,
            ebene: _,
            kuerzel: _,
            poly_alt: _,
            poly_neu
        } => {
            let o = objects.objects.get(obj_id)?;
            if o.poly.is_empty() {
                return None; // TODO: Delete non-polygon objects (attributes, AP_PTO, etc.)
            }
            Some((
                (get_replace_xml_node(
                    obj_id,
                    &o,
                    &poly_neu,
                )),
                OpPrereq::Replace { id: obj_id.clone() }
            ))
        }
    }}).collect::<Vec<_>>();

    for (id, a) in aenderungen_todo.iter().enumerate() {
        if let Operation::Insert { kuerzel, poly_neu, .. } = a {
            let obj_id_new = "DE_001".to_string() + &format!("{id:010}");
            if let Some(symbol) = Signatur::from_kuerzel(
                kuerzel, 
                &obj_id_new,
                poly_neu, 
                id * 2,
            ) {
                final_strings.push((symbol.get_xml(), OpPrereq::Insert { id: obj_id_new.clone() }));
            }
        }
    }

    final_strings    
}

pub fn process_final_strings(
    input: &[(String, OpPrereq)]
) -> Vec<(String, OpPrereq)> {
    let deletes = input.iter().filter_map(|s| match &s.1 {
        OpPrereq::Insert { id } => None,
        OpPrereq::Replace { id } => None,
        OpPrereq::Delete { id } => Some(id.clone()),
    }).collect::<BTreeSet<_>>();

    let inserts = input.iter().filter_map(|s| match &s.1 {
        OpPrereq::Delete { id } => None,
        OpPrereq::Replace { id } => None,
        OpPrereq::Insert { id } => Some(id.clone()),
    }).collect::<BTreeSet<_>>();

    let intersection = inserts.intersection(&deletes).cloned().collect::<BTreeSet<_>>();
    input.iter().filter_map(|s| if intersection.contains(&s.1.get_id()) { None } else { Some(s.clone() )}).collect()
}

pub fn filter_identic_operations(
    input: &[Operation]
) -> Vec<Operation> {

    let deletes = input.iter().filter_map(|s| match &s {
        Operation::Delete { kuerzel, poly_alt, .. } => Some(s.get_geom_kuerzel_pair()),
        Operation::Insert { .. } => None,
        Operation::Replace { .. } => None,
    }).collect::<BTreeSet<_>>();

    let inserts = input.iter().filter_map(|s| match &s {
        Operation::Delete { .. } => None,
        Operation::Insert { kuerzel, poly_neu, .. } => Some(s.get_geom_kuerzel_pair()),
        Operation::Replace { .. } => None,
    }).collect::<BTreeSet<_>>();

    let intersection = inserts.intersection(&deletes).cloned().collect::<BTreeSet<_>>();
    input.iter().filter_map(|s| if intersection.contains(&s.get_geom_kuerzel_pair()) { None } else { Some(s.clone() )}).collect()
}

pub fn operations_to_xml_file(
    aenderungen_todo: &[Operation], 
    objects: &NasXmlObjects, 
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>
) -> String {

    let final_strings = operations_to_xml_file_internal(aenderungen_todo, objects, datum_jetzt);
    let final_strings = process_final_strings(&final_strings);
    let mut final_strings = final_strings.iter().map(|s| s.0.clone()).collect::<Vec<_>>();
    final_strings.sort();

    let final_strings = final_strings.join("\r\n");

    let s = format!(
        include_str!("./antrag.xml"),
        crs = "ETRS89_UTM33",
        content = final_strings,
        profilkennung = "schuettf",
        antragsnr = "73_0073_".to_string() + &format!("{}", datum_jetzt.format("%Y%m%d")) + "_999",
    );

    s.lines()
    .filter_map(|s| if s.trim().is_empty() { None } else { Some(s.to_string()) })
    .collect::<Vec<_>>()
    .join("\r\n")
}

pub fn aenderungen_zu_nas_xml(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
    csv_data: &CsvDataType,
    objects: &NasXmlObjects,
) -> String {
    let new_nas = nas_xml.fortfuehren(aenderungen, split_nas, csv_data);
    serde_json::to_string_pretty(&new_nas).unwrap_or_default()
    // new_nas.to_xml(&nas_xml, &objects);
}

pub fn join_inserts(
    aenderungen_todo: &[Operation],
    insert_all_points: bool,
) -> Vec<Operation> {

    let mut non_insert_ops = aenderungen_todo
    .iter()
    .filter_map(|s| match s {
        Operation::Insert { .. } => None,
        _ => Some(s.clone()),
    }).collect::<Vec<_>>();

    let mut inserts_sorted_by_kuerzel = BTreeMap::new();
    for i in aenderungen_todo.iter() {
        match i {
            Operation::Insert { ebene, kuerzel, poly_neu  } => {
                inserts_sorted_by_kuerzel.entry(kuerzel.clone())
                .or_insert_with(|| (ebene.clone(), Vec::new()))
                .1.push(poly_neu.clone());
            },
            _ => { },
        }
    }

    for (k, (e, v)) in inserts_sorted_by_kuerzel.iter_mut() {
        let joined = join_polys(&v, true, insert_all_points)
        .iter()
        .flat_map(crate::nas::cleanup_poly)
        .collect::<Vec<_>>();
        *v = joined;
    }

    for (kuerzel, (ebene, polys)) in inserts_sorted_by_kuerzel.iter() {
        for p in polys.iter() {
            non_insert_ops.push(Operation::Insert { 
                ebene: ebene.clone(), 
                kuerzel: kuerzel.clone(), 
                poly_neu:  p.clone(),
            });
        }
    }

    non_insert_ops
}

// Get the na_definiert as na_polyneu
pub fn get_na_definiert_as_na_polyneu(
    aenderungen: &Aenderungen,
    split_nas: &SplitNasXml,
    fluren: &Vec<SvgPolygonInner>,
) -> Aenderungen {

    let force = true;
    let mut aenderungen = aenderungen.clone();
    let neu_objekte = aenderungen.na_definiert
        .iter()
        .filter_map(|(k, v)| Some((split_nas.get_flst_part_by_id(k)?, TaggedPolygon::get_object_id(&k)?, v)))
        .filter_map(|(k, _obj_id, v)| {

            Some((uuid(), PolyNeu {
                poly: SvgPolygon::Old(k.poly.clone()),
                nutzung: Some(v.to_string()),
                locked: true,
            }))
        })
        .collect::<BTreeMap<_, _>>();

    aenderungen.na_definiert = BTreeMap::new();
    aenderungen.na_polygone_neu = neu_objekte;

    // merge aenderungen same type first (merge adjacent flst)
    aenderungen = aenderungen.deduplicate(force);
    for _ in 0..5 {
        aenderungen = aenderungen.clean_stage25(force);
    }
    
    aenderungen = filter_aenderungen_gemarkung(&aenderungen, fluren);
    aenderungen
}

pub fn filter_aenderungen_gemarkung(
    aenderungen: &Aenderungen,
    fluren: &[SvgPolygonInner]
) -> Aenderungen {
    let mut aenderungen = aenderungen.clone();
    let newmap = aenderungen.na_polygone_neu
    .iter()
    .flat_map(|(k, v)| {
        let v_inner = v.poly.get_inner();
        fluren
        .iter()
        .flat_map(|s| intersect_polys(s, &v_inner, true))
        .collect::<Vec<_>>()
        .iter()
        .flat_map(crate::nas::cleanup_poly)
        .map(|q| (uuid(), PolyNeu { nutzung: v.nutzung.clone(), poly: SvgPolygon::Old(q.clone()), locked: v.locked }))
        .collect::<Vec<_>>()
    }).collect::<BTreeMap<_, _>>();
    for (_, v) in newmap.iter() {
        log_status(&format!("newmap: {} m2 {}", v.poly.get_inner().area_m2().round(), v.nutzung.clone().unwrap_or_default()));
    }
    aenderungen.na_polygone_neu = newmap;
    aenderungen
}

pub fn get_aenderungen_prepared(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
    fluren: &Vec<SvgPolygonInner>,
) -> Aenderungen {

    let force = true;
    let mut aenderungen = aenderungen.deduplicate(force);
    for _ in 0..5 {
        aenderungen = aenderungen.clean_stage25(force);
    }
    let aenderungen = aenderungen.clean_stage3(&split_nas,&mut Vec::new(), 0.1, 0.1, force);
    let mut aenderungen = aenderungen.deduplicate(force);

    aenderungen = filter_aenderungen_gemarkung(&aenderungen, fluren);
    // aenderungen_remove_objs_bauraum_bodenordnung(&bauraum_bodenordnung);
    aenderungen
}

pub type ReverseMap = BTreeMap<String, (String, String, TaggedPolygon, Vec<AenderungObject>)>;

/* 
pub fn filter_reverse_map(
    rm: &ReverseMap,
    fluren: &[SvgPolygonInner],
) -> ReverseMap {
    rm.into_iter().flat_map(|(de_id, (ebene, kuerzel, de_obj, aenderungen))| {
        let intersect = ;

    })
}*/

pub fn napoly_to_reverse_map(
    napoly: &BTreeMap<String, PolyNeu>,
    nas_xml: &NasXMLFile,
) -> ReverseMap {

    let mut map = BTreeMap::new();
    let alle_ebenen = crate::get_nutzungsartenkatalog_ebenen()
    .values().cloned().collect::<BTreeSet<_>>();

    for (ebene_id, tps) in nas_xml.ebenen.iter() {
        if !alle_ebenen.contains(ebene_id) {
            continue;
        }

        for tp in tps.iter() {
            let de_id = match tp.get_de_id() {
                Some(s) => s,
                None => continue,
            };
            let old_ebene = match tp.get_ebene() {
                Some(s) => s,
                None => continue,
            };
            let old_kuerzel = match tp.get_auto_kuerzel() {
                Some(s) => s,
                None => continue,
            };
            let tp_rect = tp.get_rect();
            let aenderungen = napoly.iter().filter_map(|(k, v)| {
                if v.poly.get_rect().overlaps_rect(&tp_rect) {
                    let neu_kuerzel = v.nutzung.clone()?;
                    let neu_ebene = TaggedPolygon::get_auto_ebene(&neu_kuerzel)?;
                    Some((k, (neu_kuerzel, neu_ebene, v.poly.get_inner())))
                } else {
                    None
                }
            }).collect::<Vec<_>>();

            if aenderungen.is_empty() {
                continue;
            }

            log_status(&format!("inserting napoly_to_reverse_map: {de_id} {old_ebene} {old_kuerzel}: {:?}", 
                aenderungen.iter().map(|s| format!("{} m2 {}", s.1.2.area_m2().round(), s.1.0)).collect::<Vec<_>>()
            ));

            for (k, (neu_kuerzel, neu_ebene, neu_poly)) in aenderungen {
                map
                .entry(de_id.clone())
                .or_insert_with(|| ((old_ebene.clone(), old_kuerzel.clone(), tp.clone(), Vec::new())))
                .3
                .push(AenderungObject {
                    orig_change_id: k.clone(),
                    neu_kuerzel: neu_kuerzel.clone(),
                    neu_ebene: neu_ebene,
                    poly: TaggedPolygon {
                        attributes: TaggedPolygon::get_auto_attributes_for_kuerzel(&neu_kuerzel, &[]),
                        poly: neu_poly,
                    },
                });
            }
        }
    }
    map
}

// map {DE_ID alt Objekt =?> (ebene, kürzel, taggedpolygon Aenderungen { })}
pub fn reverse_map_to_aenderungen(
    reverse_map: &ReverseMap,
    insert_all_points: bool,
) -> Vec<Operation> {
    let mut aenderungen_todo = reverse_map.iter()
    .flat_map(|(alt_obj_id, (alt_ebene, alt_kuerzel, tp, aen))| {
        
        let aenderungen_with_same_kuerzel = aen.iter().filter_map(|s| {
            if s.neu_kuerzel == *alt_kuerzel {
                log_status(&format!("alt obj id {alt_obj_id} ({alt_kuerzel}): ADDING poly {} ({} m2 {})", s.orig_change_id, s.poly.poly.area_m2().round(), s.neu_kuerzel));
                Some(s.poly.poly.clone())
            } else {
                None
            }
        }).collect::<Vec<_>>();

        log_status(&format!("reverse map to aenderungen: adding {} polys to {alt_kuerzel}", aenderungen_with_same_kuerzel.len()));

        let mut v = vec![tp.poly.clone()];
        v.extend(aenderungen_with_same_kuerzel.into_iter());
        let joined = join_polys(&v, false, insert_all_points).iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();

        let polys_to_subtract = aen.iter().filter_map(|s| {
            if s.neu_kuerzel != *alt_kuerzel {
                log_status(&format!("alt obj id {alt_obj_id} ({alt_kuerzel}): SUBTRACTING poly {} ({} m2 {})", s.orig_change_id, s.poly.poly.area_m2().round(), s.neu_kuerzel));
                Some(s)
            } else {
                None
            }
        }).collect::<Vec<_>>();

        log_status(&format!("reverse map to aenderungen: subtracting {} polys from {alt_kuerzel}", polys_to_subtract.len()));

        let subtracted = joined.iter().flat_map(|s| {
            subtract_from_poly(s, &polys_to_subtract.iter().map(|s| &s.poly.poly).collect::<Vec<_>>(), true)
        })
        .collect::<Vec<_>>()
        .iter()
        .flat_map(crate::nas::cleanup_poly)
        .collect::<Vec<_>>();

        // DELETE alt_obj_id
        // INSERT (joined) => same kuerzel
        let mut v = vec![
            Operation::Delete { 
                obj_id: alt_obj_id.clone(),
                ebene: alt_ebene.clone(), 
                kuerzel: alt_kuerzel.clone(), 
                poly_alt: tp.poly.clone() 
            },
        ];

        for s in subtracted {
            if s.is_zero_area() {
                continue;
            }
            for q in crate::nas::cleanup_poly(&s) {
                v.push(Operation::Insert { 
                    ebene: alt_ebene.clone(), 
                    kuerzel: alt_kuerzel.clone(), 
                    poly_neu: q.clone(), 
                });
            }
        }

        for q in polys_to_subtract {
            for is in intersect_polys(&tp.poly, &q.poly.poly, true) {
                v.push(Operation::Insert { 
                    ebene: q.neu_ebene.clone(), 
                    kuerzel: q.neu_kuerzel.clone(), 
                    poly_neu: is.clone(), 
                });
            }
        }

        // Insert all other objs that overlapped (will be deduplicated later)
        /*
        for q in polys_to_subtract {
            for x in subtracted.iter() {
                // so that the TN matches as a mesh
                q.insert_points_from(x, 0.05);
            }
            v.push(Operation::Delete { 
                obj_id: q, 
                ebene: (), 
                kuerzel: (), 
                poly_alt: () 
            });

            v.push(Operation::Insert { 
                ebene: (), 
                kuerzel: (), 
                poly_neu: () 
            });
        }
        */
       v
    }).collect::<Vec<_>>();

    aenderungen_todo.sort_by(|a, b| a.get_str_id().cmp(&b.get_str_id()));
    aenderungen_todo.dedup();
    log_status("JOIN INSERTS");
    // aenderungen_todo = join_inserts(&aenderungen_todo, insert_all_points);
    log_status("JOIN INSERTS DONE");
    aenderungen_todo
}
pub enum Signatur {
    Punkt {
        id: String,
        fuer: String,
        pos: SvgPoint,
        signaturnummer: String,
        art: String,
    },
    Flaeche {
        id: String,
        fuer: String,
        signaturnummer: String,
        art: String,
        positionierungsregel: String,
    }
}

impl Signatur {

    fn from_kuerzel(kuerzel: &str, obj_id: &str, poly: &SvgPolygonInner, id: usize) -> Option<Self> {
        match kuerzel {
            "SUM" => {
                let pt = poly.get_label_pos()?;
                Some(Self::Punkt { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    pos: pt, 
                    signaturnummer: "3478".to_string(), 
                    art: "Sumpf".to_string()
                })
            },
            "WAS" => {
                let pt = poly.get_label_pos()?;
                Some(Self::Punkt { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    pos: pt, 
                    signaturnummer: "3490".to_string(), 
                    art: "FKT".to_string()
                })
            },
            "WALD" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3456".to_string(), 
                    art: "VEG".to_string(), 
                    positionierungsregel: "1104".to_string()
                })
            },
            "LH" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3458".to_string(),
                    art: "VEG".to_string(), 
                    positionierungsregel: "1104".to_string()
                })
            },
            "NH" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3460".to_string(),
                    art: "VEG".to_string(), 
                    positionierungsregel: "1104".to_string()
                })
            },
            "LNH" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3462".to_string(),
                    art: "VEG".to_string(), 
                    positionierungsregel: "1104".to_string()
                })
            },
            "GR" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3413".to_string(),
                    art: "VEG".to_string(), 
                    positionierungsregel: "1100".to_string()
                })
            },
            "GRÜ" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3413".to_string(),
                    art: "FKT".to_string(), 
                    positionierungsregel: "1100".to_string()
                })
            },
            "G" => {
                Some(Self::Flaeche { 
                    id: ("DE_001".to_string() + &format!("{id:010}")), 
                    fuer: obj_id.to_string(), 
                    signaturnummer: "3421".to_string(),
                    art: "VEG".to_string(), 
                    positionierungsregel: "1100".to_string()
                })
            },
            _ => None,
        }
    }

    fn get_xml(&self) -> String {

        const PUNKTSIGNATUR: &str = r#"
        	<wfs:Insert>
				<AP_PPO gml:id="$$OBJ_ID$$">
					<gml:identifier codeSpace="http://www.adv-online.de/">urn:adv:oid:$$OBJ_ID$$</gml:identifier>
					<lebenszeitintervall>
						<AA_Lebenszeitintervall>
							<beginnt>9999-01-01T00:00:00Z</beginnt>
						</AA_Lebenszeitintervall>
					</lebenszeitintervall>
					<modellart>
						<AA_Modellart>
							<advStandardModell>DKKM1000</advStandardModell>
						</AA_Modellart>
					</modellart>
					<position>
						<gml:MultiPoint>
							<gml:pointMember>
								<gml:Point>
									<gml:pos>$$POS$$</gml:pos>
								</gml:Point>
							</gml:pointMember>
						</gml:MultiPoint>
					</position>
					<signaturnummer>$$SIGNATURNUMMER$$</signaturnummer>
					<art>$$ART$$</art>
					<dientZurDarstellungVon xlink:href="urn:adv:oid:$$FUER$$"/>
				</AP_PPO>
			</wfs:Insert>
        "#;

        const FLAECHENSIGNATUR: &str = r#"
			<wfs:Insert>
				<AP_Darstellung gml:id="$$OBJ_ID$$">
					<gml:identifier codeSpace="http://www.adv-online.de/">urn:adv:oid:$$OBJ_ID$$</gml:identifier>
					<lebenszeitintervall>
						<AA_Lebenszeitintervall>
							<beginnt>9999-01-01T00:00:00Z</beginnt>
						</AA_Lebenszeitintervall>
					</lebenszeitintervall>
					<modellart>
						<AA_Modellart>
							<advStandardModell>DKKM1000</advStandardModell>
						</AA_Modellart>
					</modellart>
					<signaturnummer>$$SIGNATURNUMMER$$</signaturnummer>
					<art>$$ART$$</art>
					<dientZurDarstellungVon xlink:href="urn:adv:oid:$$FUER$$"/>
					<positionierungsregel>$$POSITIONIERUNGSREGEL$$</positionierungsregel>
				</AP_Darstellung>
			</wfs:Insert>
        "#;

        match self {
            Signatur::Punkt { id, fuer, pos, signaturnummer, art } => {
                PUNKTSIGNATUR
                .replace("$$OBJ_ID$$", id)
                .replace("$$FUER$$", fuer)
                .replace("$$POS$$", &format!("{} {}", pos.x, pos.y))
                .replace("$$ART$$", art)
                .replace("$$SIGNATURNUMMER$$", signaturnummer)
            },
            Signatur::Flaeche { id, fuer, signaturnummer, art, positionierungsregel } => {
                FLAECHENSIGNATUR
                .replace("$$OBJ_ID$$", id)
                .replace("$$FUER$$", fuer)
                .replace("$$POSITIONIERUNGSREGEL$$", positionierungsregel)
                .replace("$$ART$$", art)
                .replace("$$SIGNATURNUMMER$$", signaturnummer)
            },
        }
    }
}

pub fn merge_aenderungen_with_existing_nas(
    aenderungen_todo: &[Operation],
    nas_xml: &NasXMLFile,
    insert_all_points: bool,
) -> Vec<Operation> {

    struct ImAenderung {
        ebene: String,
        kuerzel: String,
        poly_neu: SvgPolygonInner,
    }

    let polys = aenderungen_todo.iter().filter_map(|a| match a {
        Operation::Delete { .. } |
        Operation::Replace { .. } => None,
        Operation::Insert { ebene, kuerzel, poly_neu } => Some(ImAenderung {
            ebene: ebene.clone(),
            kuerzel: kuerzel.clone(),
            poly_neu: poly_neu.clone(),
        }),
    }).collect::<Vec<_>>();

    let attached_polys = polys.into_iter().filter_map(|s| {
        let s_rect = s.poly_neu.get_rect();
        let touching_polys = nas_xml.ebenen
        .get(&s.ebene)?
        .iter()
        .filter_map(|q| {
            if !q.get_rect().overlaps_rect(&s_rect) {
                return None;
            }
            if q.get_auto_kuerzel().as_deref() != Some(s.kuerzel.as_str()) {
                return None;
            }
            if !nas::relate(&q.poly, &s.poly_neu, 0.01).touches_other_poly_outside() {
                return None;
            }
            Some(q.clone())
        }).collect::<Vec<_>>();

        if touching_polys.is_empty() {
            None
        } else {
            Some((s.poly_neu.get_hash(), (s, touching_polys)))
        }
    }).collect::<BTreeMap<_, _>>();

    if attached_polys.is_empty() {
        return aenderungen_todo.to_vec();
    }

    let mut aenderungen_clean = aenderungen_todo.iter().filter_map(|a| match a {
        Operation::Delete { .. } |
        Operation::Replace { .. } => Some(a.clone()),
        Operation::Insert { poly_neu, ..} => if attached_polys.contains_key(&poly_neu.get_hash()) { None } else { Some(a.clone()) },
    }).collect::<Vec<_>>();

    for (_id, (im_aenderung, polys_to_join)) in attached_polys.into_iter() {
        
        let ids_to_join = polys_to_join
        .into_iter()
        .filter_map(|tp| tp.get_de_id().map(|s| (s, tp.poly)))
        .collect::<Vec<_>>();
        
        let mut polys_to_join = vec![im_aenderung.poly_neu];
        polys_to_join.extend(ids_to_join.iter().map(|a| a.1.clone()));

        let joined_poly = join_polys(&polys_to_join, false, insert_all_points);

        for j in joined_poly.into_iter() {
            aenderungen_clean.push(Operation::Insert { 
                ebene: im_aenderung.ebene.clone(), 
                kuerzel: im_aenderung.kuerzel.clone(), 
                poly_neu: j,
            });
        }
        
        for (id, poly) in ids_to_join {
            aenderungen_clean.push(Operation::Delete { 
                obj_id: id, 
                ebene: im_aenderung.ebene.clone(), 
                kuerzel: im_aenderung.kuerzel.clone(), 
                poly_alt: poly 
            });
        }
    }


    aenderungen_clean.sort_by(|a, b| a.get_str_id().cmp(&b.get_str_id()));
    aenderungen_clean.dedup();
    aenderungen_clean
}

pub fn merge_and_intersect_inserts(
    aenderungen_todo: &[Operation],
    aenderungen_to_subtract: &BTreeMap<String, PolyNeu>,
) -> Vec<Operation> {

    let mut deletes = aenderungen_todo.iter().filter_map(|op| match op {
        Operation::Delete { .. } => Some(op.clone()),
        _ => None,
    }).collect::<Vec<_>>();

    let mut insert_map = BTreeMap::new();
    for op in aenderungen_todo.iter() {
        match op {
            Operation::Insert { ebene, kuerzel, poly_neu  } => {
                insert_map.entry(kuerzel.clone()).or_insert_with(|| Vec::new()).push(poly_neu.clone()); 
            },
            _ => { },
        }
    }

    log_status("joining.... 1");
    insert_map
    .values_mut()
    .for_each(|polys| {
        log_status("joining polys 1 start...");
        *polys = crate::ops::join_polys_fast(&polys, true, true);
        log_status("joining polys 1 end...");
    });
    log_status("joining.... 2");

    // subtract defined polys from aenderungen
    let to_subtract_polys = insert_map.keys().filter_map(|k| {

        let polys_higher_order = aenderungen_to_subtract.values().filter_map(|v| {
            let kuerzel = v.nutzung.as_deref()?;
            if kuerzel != k {
                Some(v.poly.get_inner())
            } else {
                None
            }
        }).collect::<Vec<_>>();

        if polys_higher_order.is_empty() {
            None
        } else {
            Some((k.clone(), polys_higher_order))
        }
    }).collect::<BTreeMap<_, _>>();

    insert_map
    .iter_mut()
    .for_each(|(k, polys)| {
        if let Some(tosubtract) = to_subtract_polys.get(k) {
            log_status("subtracting...");
            let newpolys = polys.iter()
            .flat_map(|s| {
                subtract_from_poly(s, &tosubtract.iter().collect::<Vec<_>>(), true)
            })
            .filter_map(|p| if p.is_zero_area() { None } else { Some(p) })
            .collect::<Vec<_>>();
            log_status("subtracted!");
            *polys = newpolys;
        }
    });

    // subtract higher-order polys
    let to_subtract_polys = insert_map.keys().filter_map(|k| {
        
        let orig_nak = crate::search::get_nak_ranking(k);

        let polys_higher_order = insert_map.iter().flat_map(|(k, v)| {
            let nak = crate::search::get_nak_ranking(k);
            if nak > orig_nak {
                v.clone()
            } else {
                Vec::new()
            }
        }).collect::<Vec<_>>();

        if polys_higher_order.is_empty() {
            None
        } else {
            Some((k.clone(), polys_higher_order))
        }
    }).collect::<BTreeMap<_, _>>();

    insert_map
    .iter_mut()
    .for_each(|(k, polys)| {
        if let Some(tosubtract) = to_subtract_polys.get(k) {
            log_status("subtracting...");
            let newpolys = polys.iter()
            .flat_map(|s| {
                subtract_from_poly(s, &tosubtract.iter().collect::<Vec<_>>(), true)
            })
            .filter_map(|p| if p.is_zero_area() { None } else { Some(p) })
            .collect::<Vec<_>>();
            log_status("subtracted!");
            *polys = newpolys;
        }
    });
    
    for (kuerzel, poly) in aenderungen_to_subtract.values().filter_map(|q| q.nutzung.clone().map(|k| (k.clone(), q.poly.get_inner()))) {
        insert_map.entry(kuerzel).or_insert_with(|| Vec::new()).push(poly);
    }

    log_status("joining.... 3");
    insert_map
    .values_mut()
    .for_each(|polys| {
        log_status("joining polys 2 start...");
        *polys = crate::ops::join_polys_fast(&polys, true, true);
        log_status("joining polys 2 end...");
    });
    log_status("joining.... 4");

    deletes.extend(insert_map.into_iter().flat_map(|(kuerz, polys)| {
        let kuerz = kuerz.clone();
        polys.into_iter().filter_map(move |p| {
            Some(Operation::Insert { 
                ebene: TaggedPolygon::get_auto_ebene(&kuerz)?, 
                kuerzel: kuerz.clone(), 
                poly_neu: p 
            })
        })
    }));

    deletes
}

pub fn insert_gebaeude_delete(
    aenderungen: &Aenderungen,
    aenderungen_todo: &[Operation],
) -> Vec<Operation> {
    let mut aenderungen_todo = aenderungen_todo.to_vec();
    for g in aenderungen.gebaeude_loeschen.values() {
        aenderungen_todo.push(Operation::Delete { 
            obj_id: g.gebaeude_id.clone(), 
            ebene: "AX_Gebaeude".to_string(),
            kuerzel: String::new(), 
            poly_alt: SvgPolygonInner::default() 
        });
    }
    aenderungen_todo
}

pub fn log_aenderungen(aenderungen_todo: &[Operation]) {

    for a in aenderungen_todo.iter() {
        match a {
            Operation::Delete {
                obj_id,
                ebene: _,
                kuerzel,
                poly_alt,
            } => {
                log_status(&format!("deleting {} m2 {kuerzel} (obj {obj_id})", poly_alt.area_m2().round()));
            },
            Operation::Replace {
                obj_id,
                ebene: _,
                kuerzel,
                poly_alt,
                poly_neu,
            } => {
                log_status(&format!(
                    "replacing {} m2 {kuerzel} with {} m2 {kuerzel} (obj {obj_id})",
                    poly_alt.area_m2().round(),
                    poly_neu.area_m2().round()
                ));
            },
            Operation::Insert {
                ebene: _,
                kuerzel,
                poly_neu,
            } => {
                log_status(&format!(
                    "inserting {} m2 {kuerzel}",
                    poly_neu.area_m2().round()
                ));
            }
        }
    }
}

pub fn line_to_ring(l: &SvgLine) -> String {
    const RING_XML: &str = r#"
                                        <gml:Ring>
                                            <gml:curveMember>
                                                <gml:Curve>
                                                    <gml:segments>
                                                        <gml:LineStringSegment>
                                                            <gml:posList>$$POSLIST$$</gml:posList>
                                                        </gml:LineStringSegment>
                                                    </gml:segments>
                                                </gml:Curve>
                                            </gml:curveMember>
                                        </gml:Ring>
    "#;

    RING_XML
        .replace(
            "$$POSLIST$$",
            &l.points
                .iter()
                .map(|s| format!("{:.3} {:.3}", s.x, s.y))
                .collect::<Vec<_>>()
                .join(" "),
        )
}

pub fn polygon_to_position_node(p: &SvgPolygonInner) -> String {
    const POLY_XML: &str = r#"
                    <position>
                        <gml:Surface>
                            <gml:patches>
                                <gml:PolygonPatch>
                                    $$EXTERIOR_RINGS$$
                                    $$INTERIOR_RINGS$$
                                </gml:PolygonPatch>
                            </gml:patches>
                        </gml:Surface>
                    </position>
    "#;

    let outer_rings = Some(line_to_ring(&p.outer_ring))
        .map(|or| {
            format!("
                                    <gml:exterior>
            {or}
                                    </gml:exterior>
            ")
        }).unwrap_or_default();

    let inner_rings = p
        .inner_rings
        .iter()
        .map(|l| {
            line_to_ring(l)
        })
        .map(|ir| {
            format!("
                                    <gml:interior>
            {ir}
                                    </gml:interior>
            ")
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    POLY_XML
        .replace("$$EXTERIOR_RINGS$$", &outer_rings)
        .replace("$$INTERIOR_RINGS$$", &inner_rings)
}

pub fn get_insert_xml_node(
    ax_ebene: &str,
    obj_id: &str,
    attribute: &[(&str, &str)],
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>,
    poly: &SvgPolygonInner,
) -> String {
    const INSERT_XML: &str = r#"
            <wfs:Insert>
                <$$AX_EBENE$$ gml:id="$$OBJ_ID$$">
                    <gml:identifier codeSpace="http://www.adv-online.de/">urn:adv:oid:$$OBJ_ID$$</gml:identifier>
                    <lebenszeitintervall>
                        <AA_Lebenszeitintervall>
                            <beginnt>9999-01-01T00:00:00Z</beginnt>
                        </AA_Lebenszeitintervall>
                    </lebenszeitintervall>
                    <modellart>
                        <AA_Modellart>
                            <advStandardModell>DLKM</advStandardModell>
                        </AA_Modellart>
                    </modellart>
                    $$POSITION_NODE$$
                    <datumDerLetztenUeberpruefung>$$DATUM_JETZT$$</datumDerLetztenUeberpruefung>
                    <ergebnisDerUeberpruefung>3000</ergebnisDerUeberpruefung>
                    $$EXTRA_ATTRIBUTE$$
                </$$AX_EBENE$$>
            </wfs:Insert>
    "#;

    let attribute = attribute
        .iter()
        .map(|(k, v)| format!("<{k}>{v}</{k}>"))
        .collect::<Vec<_>>()
        .join("\r\n");

    INSERT_XML
        .replace("$$AX_EBENE$$", ax_ebene)
        .replace("$$OBJ_ID$$", obj_id)
        .replace(
            "$$POSITION_NODE$$",
            &polygon_to_position_node(poly),
        )
        .replace(
            "$$DATUM_JETZT$$",
            &datum_jetzt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        )
        .replace("$$EXTRA_ATTRIBUTE$$", &attribute)
}

pub fn get_replace_xml_node(
    obj_id: &str,
    member_object: &MemberObject,
    poly: &SvgPolygonInner,
) -> String {

    let mut attr = member_object.extra_attribute.clone();
    attr.remove("datumDerLetztenUeberpruefung");
    attr.remove("ergebnisDerUeberpruefung");
    attr.remove("identifier");
    let mut attribute = attr.iter().map(|(k, v)| {
        format!("                    <{k}>{v}</{k}>")
    }).collect::<Vec<_>>();

    let v = &[
        ("dientZurDarstellungVon", &member_object.dient_zur_darstellung_von),
        ("istBestandteilVon", &member_object.ist_bestandteil_von),
        ("hat", &member_object.hat),
        ("istTeilVon", &member_object.ist_teil_von),
    ];

    for (k, ov) in v.iter() {
        if let Some(v) = ov.as_deref() {
            attribute.push(format!("                    <{k} href=\"{v}\"/>"));
        }
    }

    const REPLACE_XML: &str = r#"
            <wfs:Replace>
                <$$EBENE$$ gml:id="$$RESOURCE_ID$$">
                    <gml:identifier codeSpace="http://www.adv-online.de/">urn:adv:oid:$$OBJECT_ID$$</gml:identifier>
                    <lebenszeitintervall>
                        <AA_Lebenszeitintervall>
                            <beginnt>$$ORIGINAL_DATE$$</beginnt>
                        </AA_Lebenszeitintervall>
                    </lebenszeitintervall>
                    <modellart>
                        <AA_Modellart>
                            <advStandardModell>DLKM</advStandardModell>
                        </AA_Modellart>
                    </modellart>
                    $$POSITION_NODE$$
$$EXTRA_ATTRIBUTE$$
                </$$EBENE$$>
                <fes:Filter>
                    <fes:ResourceId rid="$$RESOURCE_ID$$"/>
                </fes:Filter>
            </wfs:Replace>
    "#;

    let beginnt = member_object
        .beginnt
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        .replace("-", "")
        .replace(":", "");
    let rid = format!("{obj_id}{beginnt}");

    REPLACE_XML
        .replace("$$EBENE$$", &member_object.member_type)
        .replace("$$RESOURCE_ID$$", &rid)
        .replace("$$OBJECT_ID$$", obj_id)
        .replace("$$ORIGINAL_DATE$$", &member_object.beginnt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .replace(
            "$$POSITION_NODE$$",
            &polygon_to_position_node(poly),
        )
        .replace("$$EXTRA_ATTRIBUTE$$", &attribute.join("\r\n"))
}
