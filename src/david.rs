use crate::{
    nas::{
        self, MemberObject, NasXMLFile, NasXmlObjects, SplitNasXml, SvgLine, SvgPolygonInner, TaggedPolygon
    },
    pdf::{
        join_polys,
        subtract_from_poly,
    },
    ui::Aenderungen,
    uuid_wasm::{
        log_status, log_status_clear, uuid
    },
};
use std::collections::{
    BTreeMap,
    BTreeSet,
};

struct TempOverlapObject {
    neu_kuerzel: String,
    neu_ebene: String,
    poly: TaggedPolygon,
    overlaps_objekte: BTreeMap<String, Vec<TaggedPolygon>>,
}

struct AenderungObject {
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
}

pub fn aenderungen_zu_fa_xml(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
    objects: &NasXmlObjects,
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>,
) -> String {

    log_status_clear();

    let aenderungen_todo = get_aenderungen_internal(aenderungen, nas_xml, split_nas);
    
    log_aenderungen(&aenderungen_todo);

    let aenderungen_todo = merge_aenderungen_with_existing_nas(
        &aenderungen_todo,
        &nas_xml,
    );

    log_status("--------");

    log_aenderungen(&aenderungen_todo);

    log_status("done!");

    let mut final_strings = aenderungen_todo.iter()
    .enumerate()
    .filter_map(|(i, s)| {
        match s {
        Operation::Delete { obj_id, .. } => {
            let o = objects.objects.get(obj_id)?;
            if o.poly.is_none() {
                return None; // TODO: Delete non-polygon objects (attributes, AP_PTO, etc.)
            }
            let beginnt = o.beginnt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true).replace("-", "").replace(":", "");
            let rid = format!("{obj_id}{beginnt}");
            let typename = &o.member_type;
            Some(format!("            <wfs:Delete typeName=\"{typename}\"><fes:Filter><fes:ResourceId rid=\"{rid}\" /></fes:Filter></wfs:Delete>"))
        },
        Operation::Insert { ebene, kuerzel, poly_neu } => {
            let mut auto_attribute = TaggedPolygon::get_auto_attributes_for_kuerzel(&kuerzel, &[]);
            auto_attribute.remove("AX_Ebene");
            let auto_attribute = auto_attribute.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect::<Vec<_>>();
            Some(get_insert_xml_node(
                ebene,
                &("DE_001".to_string() + &format!("{i:010}")),  // TODO
                &auto_attribute,
                datum_jetzt,
                poly_neu,
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
            if o.poly.is_none() {
                return None; // TODO: Delete non-polygon objects (attributes, AP_PTO, etc.)
            }
            Some(get_replace_xml_node(
                obj_id,
                &o,
                &poly_neu,
            ))
        }
    }}).collect::<Vec<_>>();

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
    objects: &NasXmlObjects,
) -> String {

    log_status_clear();
    let a_internal = get_aenderungen_internal(aenderungen, nas_xml, split_nas);
    // process operations in NAS file
    format!("TODO!")
}

fn get_aenderungen_internal(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
) -> Vec<Operation> {

    let alle_ebenen = crate::get_nutzungsartenkatalog_ebenen();

    let qt = nas_xml.create_quadtree();
        
    let mut ids_to_change_nutzungen = aenderungen
        .na_definiert
        .iter()
        .filter_map(|(k, v)| Some((split_nas.get_flst_part_by_id(k)?, TaggedPolygon::get_object_id(&k)?, v)))
        .filter_map(|(k, obj_id, v)| {
            let (ebene, tp) = nas_xml.ebenen.iter().find_map(|(ebene, items)| {
                items
                    .iter()
                    .find(|s| s.get_de_id().as_deref() == Some(obj_id.as_str()))
                    .map(|tp| (ebene.clone(), tp.clone()))
            })?;

            let neu_kuerzel = v.to_string();
            let neu_ebene = TaggedPolygon::get_auto_ebene(&neu_kuerzel)?;

            log_status(&format!("1: inserting {} m2 {neu_kuerzel}", k.poly.area_m2()));

            Some((
                uuid(),
                TempOverlapObject {
                    neu_ebene,
                    neu_kuerzel,
                    poly: tp.clone(),
                    overlaps_objekte: vec![(ebene, vec![TaggedPolygon {
                        attributes: tp.attributes.clone(),
                        poly: k.poly.clone(),
                    }])].into_iter().collect(),
                },
            ))
        })
        .collect::<BTreeMap<_, _>>();

    ids_to_change_nutzungen.extend(aenderungen.na_polygone_neu.iter().filter_map(
        |(k, polyneu)| {
            let neu_kuerzel = polyneu.nutzung.clone()?;
            let neu_ebene = TaggedPolygon::get_auto_ebene(&neu_kuerzel)?;
            let poly = polyneu.poly.get_inner();
            let overlapping_objekte = qt.get_overlapping_ebenen(&poly, &alle_ebenen);
            let mut map = BTreeMap::new();
            for (k, v) in overlapping_objekte {
                log_status(&format!("2: inserting {} m2 {}", v.poly.area_m2(), neu_kuerzel));
                map.entry(k).or_insert_with(|| Vec::new()).push(v);
            }
            Some((
                k.clone(),
                TempOverlapObject {
                    neu_ebene,
                    neu_kuerzel,
                    poly: TaggedPolygon {
                        attributes: BTreeMap::new(),
                        poly,
                    },
                    overlaps_objekte: map,
                },
            ))
        },
    ));

    log_status("---- 1 ---- start");
    for (k, v) in ids_to_change_nutzungen.iter() {
        let overlaps = v.overlaps_objekte.values()
        .flat_map(|s| s.iter().map(|q| format!("{} m2 {}", q.poly.area_m2(), q.get_auto_kuerzel().unwrap_or_default())))
        .collect::<Vec<_>>();
        log_status(&format!("{k}: {} m2 {}: overlaps / touches {:?}", v.poly.poly.area_m2(), v.neu_kuerzel, overlaps));
    }
    log_status("---- 1 ---- end");

    // TODO: first join TempOverlapObject wenn ebenen gleich sind!

    // build reverse map (obj id -> relevant changes per changed obj)
    let mut reverse_map = BTreeMap::new();
    for (k, v) in ids_to_change_nutzungen.iter() {
        for (_, k2) in v.overlaps_objekte.iter() {
            for tp in k2.iter() {
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
                reverse_map
                    .entry((de_id, old_ebene, old_kuerzel))
                    .or_insert_with(|| (tp.clone(), Vec::new()))
                    .1
                    .push(AenderungObject {
                        orig_change_id: k.clone(),
                        neu_kuerzel: v.neu_kuerzel.clone(),
                        neu_ebene: v.neu_ebene.clone(),
                        poly: v.poly.clone(),
                    });
            }
        }
    }

    log_status("---- 2 ---- reverse_map start");
    for (k, v) in reverse_map.iter() {
        let overlaps = v.1.iter()
        .map(|s| format!("{} m2 {}", s.poly.poly.area_m2(), s.poly.get_auto_kuerzel().unwrap_or_default()))
        .collect::<Vec<_>>();
        log_status(&format!("{} ({} m2 {}): intersect or join with: {:?}", k.0, v.0.poly.area_m2(), k.2, overlaps));
    }
    log_status("---- 2 ---- reverse_map end");

    let mut aenderungen_todo = Vec::new();

    // depending on neu_ebene, either join (if ebene is same) or subtract (if ebene is different)
    for ((alt_obj_id, alt_ebene, alt_kuerzel), (tp, aenderungen)) in reverse_map.iter() {
        
        let aenderungen_joined = aenderungen_todo
            .iter()
            .filter_map(|s| match s {
                Operation::Replace { obj_id, .. } | Operation::Delete { obj_id, .. } => {
                    Some(obj_id)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        let polys_to_add = aenderungen
            .iter()
            .filter_map(|a| {
                let relate = nas::relate(&a.poly.poly, &tp.poly, 0.02);
                if a.neu_kuerzel == *alt_kuerzel
                    && (relate.touches_other_poly_outside() || relate.overlaps())
                    && !aenderungen_joined.contains(&a.orig_change_id)
                {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let tp_poly = if !polys_to_add.is_empty() {
            let polys_to_join = vec![tp.poly.clone()];
            if let Some(joined) = join_polys(&polys_to_join, false, false) {
                if joined.area_m2() == tp.poly.area_m2() {
                    vec![tp.poly.clone()]
                } else {
                    joined.recombine_polys()
                }
            } else {
                vec![tp.poly.clone()]
            }
        } else {
            vec![tp.poly.clone()]
        };

        let final_joined_polys = tp_poly
            .iter()
            .map(|jp| {

                let polys_to_subtract = reverse_map
                    .iter()
                    .flat_map(|(k, a)| {
                        a.1.iter().filter_map(|s| {
                            if s.neu_kuerzel != *alt_kuerzel {
                                Some(s)
                            } else {
                                None
                            }
                        }).collect::<Vec<_>>()
                    })
                    .filter_map(|a| {
                        let relate = nas::relate(&a.poly.poly, &jp, 0.02);
                        if relate.touches_other_poly_outside() 
                           || relate.overlaps()
                           || relate.a_contained_in_b() 
                           || relate.b_contained_in_a() 
                           || a.poly.get_de_id() == Some(alt_obj_id.clone()) 
                           || a.poly.poly.get_hash() == jp.get_hash() {
                            Some(a)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let p_subtract = polys_to_subtract
                    .iter()
                    .map(|p| &p.poly.poly)
                    .collect::<Vec<_>>();
                let subtracted = subtract_from_poly(&tp.poly, &p_subtract);

                (jp, subtracted, polys_to_subtract)
            })
            .collect::<Vec<_>>();

        if final_joined_polys.len() == 1 {
            let (jp, subtracted, polys_to_subtract) = &final_joined_polys[0];
            let jp_area_m2 = jp.area_m2();

            if subtracted.is_zero_area() {
                aenderungen_todo.push(Operation::Delete {
                    obj_id: alt_obj_id.clone(),
                    ebene: alt_ebene.clone(),
                    kuerzel: alt_kuerzel.clone(),
                    poly_alt: (*jp).clone(),
                });
                for a in polys_to_subtract.iter() {
                    aenderungen_todo.push(Operation::Insert {
                        ebene: a.neu_ebene.clone(),
                        kuerzel: a.neu_kuerzel.clone(),
                        poly_neu: a.poly.poly.correct_winding_order_cloned(),
                    });
                }
            } else if subtracted.area_m2().round() < jp_area_m2 {
                // original polygon did change, area is now less but not zero: modify obj to be now
                // subtracted
                aenderungen_todo.push(Operation::Replace {
                    obj_id: alt_obj_id.clone(),
                    ebene: alt_ebene.clone(),
                    kuerzel: alt_kuerzel.clone(),
                    poly_alt: tp.poly.clone(),
                    poly_neu: subtracted.correct_winding_order_cloned(),
                });
                for s in polys_to_subtract.iter() {
                    aenderungen_todo.push(Operation::Insert { 
                        ebene: s.neu_ebene.clone(),
                        kuerzel: s.neu_kuerzel.clone(),
                        poly_neu: s.poly.poly.correct_winding_order_cloned(),
                    });
                }
            } else {
                // original polygon did not change: subtractions were likely outside / touching
                log_status(&format!("{}: original polygon did not change!!! original area = {} m2, subtracted area = {} m2 (polys to subtract = {})", 
                    tp.get_de_id().unwrap_or_default(),
                    jp_area_m2.round(),
                    subtracted.area_m2().round(),
                    polys_to_subtract.len(),
                ));
            }
        } else {
            // delete original object, replace with all remaining ones
            let polys_final = final_joined_polys
                .iter()
                .filter_map(|s| {
                    if s.1.is_zero_area() {
                        None
                    } else {
                        Some(s.1.clone())
                    }
                })
                .collect::<Vec<_>>();

            aenderungen_todo.push(Operation::Delete {
                obj_id: alt_obj_id.clone(),
                ebene: alt_ebene.clone(),
                kuerzel: alt_kuerzel.clone(),
                poly_alt: tp.poly.clone(),
            });

            for p in polys_final {
                aenderungen_todo.push(Operation::Insert {
                    ebene: alt_ebene.clone(),
                    kuerzel: alt_kuerzel.clone(),
                    poly_neu: p.correct_winding_order_cloned(),
                });
            }
        }
    }

    aenderungen_todo.sort_by(|a, b| a.get_str_id().cmp(&b.get_str_id()));
    aenderungen_todo.dedup();
    aenderungen_todo
}

fn merge_aenderungen_with_existing_nas(
    aenderungen_todo: &[Operation],
    nas_xml: &NasXMLFile
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

        let joined_poly = match join_polys(&polys_to_join, false, false) {
            Some(s) => s,
            None => continue,
        };

        aenderungen_clean.push(Operation::Insert { 
            ebene: im_aenderung.ebene.clone(), 
            kuerzel: im_aenderung.kuerzel.clone(), 
            poly_neu: joined_poly 
        });
        
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

fn log_aenderungen(aenderungen_todo: &[Operation]) {

    for a in aenderungen_todo.iter() {
        match a {
            Operation::Delete {
                obj_id,
                ebene: _,
                kuerzel,
                poly_alt,
            } => {
                log_status(&format!("deleting {} m2 {kuerzel} (obj {obj_id})", poly_alt.area_m2()));
            }
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
            Operation::Replace {
                obj_id: _,
                ebene: _,
                kuerzel,
                poly_alt,
                poly_neu,
            } => {
                log_status(&format!(
                    "replacing {} m2 {kuerzel} with {} m2 {kuerzel}",
                    poly_alt.area_m2().round(),
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

    let outer_rings = p
        .outer_rings
        .iter()
        .map(|l| {
            line_to_ring(l)
        })
        .map(|or| {
            format!("
            <gml:exterior>
            {or}
            </gml:exterior>
            ")
        })
        .collect::<Vec<_>>()
        .join("\r\n");

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
