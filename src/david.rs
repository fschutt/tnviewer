use crate::{
    nas::{
        self,
        MemberObject,
        NasXMLFile,
        NasXmlObjects,
        SvgLine,
        SvgPolygonInner,
        TaggedPolygon,
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

pub fn line_to_ring(l: &SvgLine, line_id: &str) -> String {
    const RING_XML: &str = r#"
                                        <gml:Ring>
                                            <gml:curveMember>
                                                <gml:Curve gml:id="$$CURVEID$$">
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
                .map(|s| format!("{} {}", s.x, s.y))
                .collect::<Vec<_>>()
                .join(" "),
        )
        .replace("$$CURVEID$$", line_id)
}

pub fn polygon_to_position_node(p: &SvgPolygonInner, poly_id: &str) -> String {
    const POLY_XML: &str = r#"
                    <position>
                        <gml:Surface gml:id="$$POLY_ID$$">
                            <gml:patches>
                                <gml:PolygonPatch>
                                    $$EXTERIOR_RINGS$$
                                    $$INTERIOR_RINGS$$
                                </gml:PolygonPatch>
                            </gml:patches>
                        </gml:Surface>
                    </position>
    "#;

    let mut line_id = 0;

    let outer_rings = p
        .outer_rings
        .iter()
        .map(|l| {
            line_id += 1;
            line_to_ring(l, &number_to_alphabet_value(line_id))
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    let outer_rings = if outer_rings.is_empty() { 
        outer_rings 
    } else {
        format!("
                                    <gml:exterior>
                                    {outer_rings}
                                    </gml:exterior>
        ")
    };

    let inner_rings = p
        .inner_rings
        .iter()
        .map(|l| {
            line_id += 1;
            line_to_ring(l, &number_to_alphabet_value(line_id))
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    let inner_rings = if inner_rings.is_empty() { 
        inner_rings 
    } else {
        format!("
                                    <gml:interior>
                                    {inner_rings}
                                    </gml:interior>
        ")
    };

    POLY_XML
        .replace("$$EXTERIOR_RINGS$$", &outer_rings)
        .replace("$$INTERIOR_RINGS$$", &inner_rings)
        .replace("$$POLY_ID$$", poly_id)
}

pub fn get_insert_xml_node(
    ax_ebene: &str,
    obj_id: &str,
    attribute: &[(&str, &str)],
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>,
    poly: &SvgPolygonInner,
    poly_id: &str,
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
            &polygon_to_position_node(poly, poly_id),
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
    poly_id: &str,
) -> String {
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
            &polygon_to_position_node(poly, poly_id),
        )
}
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


pub fn aenderungen_zu_nas_xml(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    objects: &NasXmlObjects,
) -> String {

    log_status_clear();
    let a_internal = get_aenderungen_internal(aenderungen, nas_xml);
    // process operations in NAS file
    format!("TODO!")
}

fn get_aenderungen_internal(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
) -> Vec<Operation> {

    let alle_ebenen = crate::get_nutzungsartenkatalog_ebenen();

    let qt = nas_xml.create_quadtree();

    let mut ids_to_change_nutzungen = aenderungen
        .na_definiert
        .iter()
        .filter_map(|(k, v)| Some((TaggedPolygon::get_object_id(&k)?, v)))
        .filter_map(|(obj_id, v)| {
            let (ebene, tp) = nas_xml.ebenen.iter().find_map(|(ebene, items)| {
                items
                    .iter()
                    .find(|s| s.get_de_id().as_deref() == Some(obj_id.as_str()))
                    .map(|tp| (ebene.clone(), tp.clone()))
            })?;

            let neu_kuerzel = v.to_string();
            let neu_ebene = TaggedPolygon::get_auto_ebene(&neu_kuerzel)?;

            Some((
                uuid(),
                TempOverlapObject {
                    neu_ebene,
                    neu_kuerzel,
                    poly: tp.clone(),
                    overlaps_objekte: vec![(ebene, vec![tp])].into_iter().collect(),
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

    let mut aenderungen_todo = Vec::new();

    // depending on neu_ebene, either join (if ebene is same) or subtract (if ebene is different)
    for ((alt_obj_id, alt_ebene, alt_kuerzel), (tp, aenderungen)) in reverse_map {
        
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
                if a.neu_kuerzel == alt_kuerzel
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

                let polys_to_subtract = aenderungen
                    .iter()
                    .filter_map(|a| {
                        if a.neu_kuerzel != alt_kuerzel {
                            Some(a)
                        } else {
                            None
                        }
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
                    obj_id: alt_obj_id,
                    ebene: alt_ebene,
                    kuerzel: alt_kuerzel,
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
                    obj_id: alt_obj_id,
                    ebene: alt_ebene,
                    kuerzel: alt_kuerzel,
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

pub fn aenderungen_zu_fa_xml(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    objects: &NasXmlObjects,
    datum_jetzt: &chrono::DateTime<chrono::FixedOffset>,
) -> String {

    log_status_clear();

    let aenderungen_todo = get_aenderungen_internal(aenderungen, nas_xml);
    
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

    let mut final_strings = aenderungen_todo.iter()
    .filter_map(|s| {
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
                &uuid().replace("-", "").to_ascii_uppercase(),  // TODO
                &auto_attribute,
                datum_jetzt,
                poly_neu,
                &uuid().replace("-", "").to_ascii_uppercase(), // TODO
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
                &uuid().replace("-", "").to_ascii_uppercase(), // TODO
            ))
        }
    }}).collect::<Vec<_>>();

    final_strings.sort();
    let final_strings = final_strings.join("\r\n");

    let s = format!(
        include_str!("./antrag.xml"),
        crs = "",
        content = final_strings,
        profilkennung = "",
        antragsnr = ""
    );

    s.lines()
    .filter_map(|s| if s.trim().is_empty() { None } else { Some(s.to_string()) })
    .collect::<Vec<_>>()
    .join("\r\n")
}

/// Maps an index number to a value, i.e.:
///
/// ```no_run,ignore
/// 0   -> A
/// 25  -> Z
/// 26  -> AA
/// 27  -> AB
/// ```
///
/// ... and so on
pub fn number_to_alphabet_value(num: usize) -> String {
    const ALPHABET_LEN: usize = 26;
    // usize::MAX is "GKGWBYLWRXTLPP" with a length of 15 characters
    const MAX_LEN: usize = 15;

    let mut result = [0; MAX_LEN];

    // How many times does 26 fit in the target number?
    let mut multiple_of_alphabet = num / ALPHABET_LEN;
    let mut counter = 0;

    while multiple_of_alphabet != 0 && counter < MAX_LEN {
        let remainder = (multiple_of_alphabet - 1) % ALPHABET_LEN;
        result[(MAX_LEN - 1) - counter] = u8_to_char(remainder as u8);
        counter += 1;
        multiple_of_alphabet = (multiple_of_alphabet - 1) / ALPHABET_LEN;
    }

    let len = MAX_LEN.saturating_sub(counter);
    // Reverse the current characters
    let mut result = result[len..MAX_LEN]
        .iter()
        .map(|c| *c as char)
        .collect::<String>();

    // Push the last character
    result.push(u8_to_char((num % ALPHABET_LEN) as u8) as char);

    result
}

#[inline(always)]
fn u8_to_char(input: u8) -> u8 {
    'A' as u8 + input
}
