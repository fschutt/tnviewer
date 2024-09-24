use crate::{
    nas::{
        NasXMLFile,
        NasXmlObjects,
        SvgLine,
        SvgPolygonInner,
        TaggedPolygon,
    },
    ui::Aenderungen,
    uuid_wasm::uuid,
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

    let inner_rings = p
        .inner_rings
        .iter()
        .map(|l| {
            line_id += 1;
            line_to_ring(l, &number_to_alphabet_value(line_id))
        })
        .collect::<Vec<_>>()
        .join("\r\n");

    POLY_XML
        .replace("$$EXTERIOR_RINGS$$", &outer_rings)
        .replace("$$INTERIOR_RINGS$$", &inner_rings)
        .replace("$$POLY_ID$$", poly_id)
}

pub fn get_insert_xml_node(
    ax_ebene: &str,
    obj_id: &str,
    attribute: &[(&str, &str)],
    datum_jetzt: chrono::DateTime<chrono::FixedOffset>,
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

struct TempOverlapObject {
    neu_kuerzel: String,
    neu_ebene: String,
    poly: SvgPolygonInner,
    overlaps_objekte: BTreeMap<String, Vec<TaggedPolygon>>,
}

pub fn aenderungen_zu_fa_xml(
    aenderungen: &Aenderungen,
    nas_xml: &NasXMLFile,
    objects: &NasXmlObjects,
) -> String {
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
                    poly: tp.poly.clone(),
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
                    poly,
                    overlaps_objekte: map,
                },
            ))
        },
    ));

    let replace_obj_ids = ids_to_change_nutzungen
        .values()
        .flat_map(|overlap| {
            overlap.overlaps_objekte.iter().flat_map(|(ebene, objs)| {
                objs.iter()
                    .filter_map(|o| o.get_de_id())
                    .map(|obj_id| (ebene.clone(), obj_id.clone()))
            })
        })
        .collect::<BTreeSet<(String, String)>>();

    /*
        get_insert_xml_node(
            ax_ebene: &str,
            obj_id: &str,
            attribute: &[(&str, &str)],
            datum_jetzt: chrono::DateTime<chrono::FixedOffset>,
            poly: &SvgPolygonInner,
            poly_id: &str,
        )
    */

    let delete_string = replace_obj_ids.iter().filter_map(|(_ebene, obj_id)| {
        let o = objects.objects.get(obj_id)?;
        if o.poly.is_none() {
            return None; // TODO: Delete non-polygon objects (attributes, AP_PTO, etc.)
        }
        let beginnt = o.beginnt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true).replace("-", "");
        let rid = format!("{obj_id}{beginnt}");
        let typename = &o.member_type;
        Some(format!("\r\n            <wfs:Delete typeName=\"{typename}\"><fes:Filter><fes:ResourceId rid=\"{rid}\" /></fes:Filter></wfs:Delete>"))
    }).collect::<Vec<_>>().join("\r\n");

    let insert_string = "";

    format!(
        include_str!("./antrag.xml"),
        crs = "",
        wfs_delete = delete_string,
        wfs_replace = "",
        wfs_insert = insert_string,
        profilkennung = "",
        antragsnr = ""
    )
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
