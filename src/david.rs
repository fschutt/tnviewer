use crate::{
    csv::CsvDataType,
    nas::{
        NasXMLFile,
        NasXmlObjects,
        TaggedPolygon,
    },
    pdf::join_polys,
    ui::{
        Aenderungen,
        AenderungenIntersection,
    },
    SplitNasXml,
};
use std::collections::{
    BTreeMap,
    BTreeSet,
};

pub fn aenderungen_zu_fa_xml(
    aenderungen: &Aenderungen,
    split_nas: &SplitNasXml,
    nas_xml: &NasXMLFile,
    csv_data: &CsvDataType,
    objects: &NasXmlObjects,
) -> String {
    let splitflaechen =
        crate::geograf::calc_splitflaechen(&aenderungen, split_nas, nas_xml, &csv_data);

    let ids_to_delete = splitflaechen
        .0
        .iter()
        .filter_map(|s| {
            let oid = s.get_object_id()?;
            // TODO: hat, ist_teil_von, etc. !!!
            Some(oid)
        })
        .collect::<BTreeSet<_>>();

    let mut insert = BTreeMap::new();
    splitflaechen
        .0
        .iter()
        .filter_map(|s| {
            let ebene = TaggedPolygon::get_auto_ebene(&s.neu)?;
            Some((ebene, s))
        })
        .for_each(|(ebene, s)| {
            insert
                .entry(ebene.to_string())
                .or_insert_with(|| Vec::new())
                .push(s.clone());
        });

    for (ebene, v) in nas_xml.ebenen.iter() {
        if !insert.contains_key(ebene) {
            continue;
        }

        for s in v.iter().filter_map(|tp| {
            let flst_part_id = tp.get_flst_part_id()?;
            if ids_to_delete.contains(&flst_part_id) {
                return None;
            }
            let kuerzel = tp.get_auto_kuerzel()?;
            Some(AenderungenIntersection {
                alt: kuerzel.clone(),
                neu: kuerzel,
                flst_id: tp.get_flurstueck_id()?,
                flst_id_part: flst_part_id,
                poly_cut: tp.poly.clone(),
            })
        }) {
            insert
                .entry(ebene.clone())
                .or_insert_with(|| Vec::new())
                .push(s);
        }
    }

    // TODO: join all objects by ebene and split / recombine by outer ring
    let _ebenen = nas_xml
        .ebenen
        .iter()
        .filter_map(|(ebene, v)| {
            let svgs = v.iter().map(|s| s.poly.clone()).collect::<Vec<_>>();
            let joined = join_polys(&svgs, false, false)?;
            Some((ebene.clone(), joined))
        })
        .collect::<Vec<_>>();

    /*
    for (ebene, v) in ebenen {
        let string = polygon_to_xml(v);
        format!("<wfs:Insert>{}</wfs:Insert>")
    }
    */

    // TODO: now compare the geometry: if any object is the same, neither delete nor insert it

    let delete_string = ids_to_delete.iter().filter_map(|id| {
        let o = objects.objects.get(id)?;
        if o.poly.is_none() {
            return None; // TODO: Delete non-polygon objects (attributes, AP_PTO, etc.)
        }
        let beginnt = o.beginnt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let rid = format!("{id}{beginnt}");
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
