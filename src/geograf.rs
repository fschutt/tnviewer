use std::{collections::{BTreeMap, BTreeSet}, io::BufWriter, path::PathBuf};

use dxf::{Vector, XData, XDataItem};
use printpdf::{BuiltinFont, CustomPdfConformance, IndirectFontRef, Mm, PdfConformance, PdfDocument};
use quadtree_f32::Rect;
use wasm_bindgen::JsValue;
use web_sys::js_sys::JsString;

use crate::{nas::{NasXMLFile, SplitNasXml, SvgLine, SvgPoint, LATLON_STRING}, pdf::{Konfiguration, ProjektInfo, RissMap, Risse}, search::NutzungsArt, ui::{Aenderungen, AenderungenClean, AenderungenIntersection, TextPlacement}, xlsx::FlstIdParsed, zip::write_files_to_zip};

/// Returns the dxf bytes
pub fn texte_zu_dxf_datei(texte: &[TextPlacement]) -> Vec<u8> {
    use dxf::Drawing;
    use dxf::entities::*;

    let mut drawing = Drawing::new();

    fn update_x(zone: usize, pos: f64) -> f64 {
        format!("{zone}{pos}").parse().unwrap_or_default()
    }

    for text in texte {
        let newx = update_x(33, text.pos.x);
        let entity = Entity::new(EntityType::Text(dxf::entities::Text {
            thickness: 0.0,
            location: dxf::Point { x: newx, y: text.pos.y, z: 0.0 },
            text_height: 5.0,
            value: text.kuerzel.clone(),
            rotation: 0.0,
            relative_x_scale_factor: 1.0,
            oblique_angle: 0.0,
            text_style_name: match text.status {
                crate::ui::TextStatus::Old => "old",
                crate::ui::TextStatus::New => "new",
                crate::ui::TextStatus::StaysAsIs => "stayasis",
            }.to_string(),
            text_generation_flags: 0,
            second_alignment_point: dxf::Point::origin(),
            normal: Vector::z_axis(),
            horizontal_text_justification: dxf::enums::HorizontalTextJustification::Center,
            vertical_text_justification: dxf::enums::VerticalTextJustification::Middle,
        }));
        let _entity_ref = drawing.add_entity(entity);
    }

    let v = Vec::new();
    let mut buf = BufWriter::new(v);
    let _ = drawing.save(&mut buf);
    buf.into_inner().unwrap_or_default()
}

pub struct ShpReturn {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
    pub prj: Vec<u8>,
    pub cpg: Vec<u8>,
}

pub fn lines_to_shp(lines: &[SvgLine]) -> ShpReturn {
    
    use std::io::Cursor;
    use shapefile::{dbase::{FieldName, Record}, Point};
    use shapefile::record::polyline::Polyline;


    let mut shp_dest = Cursor::new(Vec::<u8>::new());
    let mut shx_dest = Cursor::new(Vec::<u8>::new());
    let mut dbf_dest = Cursor::new(Vec::<u8>::new());

    let shape_writer = shapefile::ShapeWriter::with_shx(&mut shp_dest, &mut shx_dest);

    let dbase_writer = dbase::TableWriterBuilder::new()
        // .add_character_field(FieldName::try_from("KUERZ").unwrap(), 10)
        // .add_character_field(FieldName::try_from("POLYID").unwrap(), 50)
        .build_with_dest(&mut dbf_dest);

    let shape_writer = shapefile::Writer::new(shape_writer, dbase_writer);

    let mut shapes = Vec::new();
    for l in lines {
        if l.points.is_empty() {
            continue;
        }

        shapes.push((
            Polyline::new(l.points.iter().map(|p| Point::new(p.x, p.y)).collect()), 
            Record::default(),
        ));
    }

    let shapes_ref = shapes.iter().map(|(a, b)| (a, b)).collect::<Vec<_>>();

    let _ = shape_writer.write_shapes_and_records(shapes_ref.into_iter());

    // https://www.geoportal-mv.de/portal/downloads/prj/25833.prj
    let prj = include_str!("./25833.prj");

    ShpReturn {
        shp: shp_dest.into_inner(),
        shx: shx_dest.into_inner(),
        dbf: dbf_dest.into_inner(),
        prj: prj.as_bytes().to_vec(),
        cpg: "UTF-8".as_bytes().to_vec(),
    }
}

pub fn export_aenderungen_geograf(
    split_nas: &SplitNasXml, // original projection
    nas_xml: &NasXMLFile, // original projection
    projekt_info: &ProjektInfo,
    konfiguration: &Konfiguration,
    aenderungen: &Aenderungen, 
    risse: &Risse,
    risse_extente: &RissMap,
) -> Vec<u8> {

    let mut files = Vec::new();

    // RISSE -> risse.shp
    if !risse.is_empty() {
        append_shp(&mut files, "RISSE", None, generate_risse_shp(&risse_extente, &split_nas.crs));
    }

    // Anschlussrisse PDFs
    let len = risse.len();
    for (i, _) in risse.iter().enumerate() {
        let i = i + 1;
        let pdf_horz = generate_anschlussriss_pdf(i, len, false);
        let pdf_vert = generate_anschlussriss_pdf(i, len, true);
        files.push((None, format!("HORZ_{i}_von_{len}.pdf").into(), pdf_horz));
        files.push((None, format!("VERT_{i}_von_{len}.pdf").into(), pdf_vert));
    }

    let splitflaechen = calc_splitflaechen(aenderungen, split_nas);

    if risse.is_empty() {
        export_splitflaechen(
            &mut files, 
            projekt_info, 
            None, 
            &splitflaechen, 
            &split_nas, 
            &nas_xml,
            None
        );
    } else {
        for (id, r) in risse.iter() {
            let ex = risse_extente.get(id).and_then(|r| r.reproject(&split_nas.crs, &mut Vec::new()));
            let extent = match ex {
                Some(s) => s,
                None => continue,
            };
            let extent_rect = extent.get_rect();
            let splitflaechen_for_riss = splitflaechen
                .iter()
                .filter(|s| s.poly_cut.get_rect().overlaps_rect(&extent_rect)).cloned()
                .collect::<Vec<_>>();
            
            export_splitflaechen(
                &mut files, 
                projekt_info, 
                Some(id.to_string()), 
                &splitflaechen_for_riss, 
                &split_nas, 
                &nas_xml,
                Some(extent_rect.clone())
            );
        }
    }

    write_files_to_zip(&files)
}

pub fn calc_splitflaechen(
    aenderungen: &Aenderungen,
    split_nas: &SplitNasXml,
) -> Vec<AenderungenIntersection> {

    let aenderungen = aenderungen
        .clean_stage4(split_nas, &mut Vec::new())
        .clean_stage5(split_nas, &mut Vec::new());

    let qt = split_nas.create_quadtree();

    let aenderungen_merged_by_typ = aenderungen.na_polygone_neu.values()
    .filter_map(|polyneu| Some((polyneu.nutzung.clone()?, polyneu.poly.clone())))
    .collect::<BTreeMap<_, _>>();

    let aenderungen = AenderungenClean {
        nas_xml_quadtree: qt,
        map: aenderungen_merged_by_typ,
    };

    aenderungen.get_aenderungen_intersections()
}

pub fn generate_risse_shp(
    riss_map: &RissMap,
    target_crs: &str,
) -> ShpReturn {
    lines_to_shp(&riss_map.iter().filter_map(|(id, re)| {
        let reproject = re.reproject(target_crs, &mut Vec::new())?;
        let rect = reproject.get_rect();
        Some(SvgLine {
            points: vec![
                SvgPoint {
                    x: rect.min_x,
                    y: rect.min_y,
                },
                SvgPoint {
                    x: rect.min_x,
                    y: rect.max_y,
                },
                SvgPoint {
                    x: rect.max_x,
                    y: rect.max_y,
                },
                SvgPoint {
                    x: rect.max_x,
                    y: rect.min_y,
                },
                SvgPoint {
                    x: rect.min_x,
                    y: rect.min_y,
                },
            ]
        })
    }).collect::<Vec<_>>())
}

pub fn export_splitflaechen(
    files: &mut Vec<(Option<String>, PathBuf, Vec<u8>)>,
    info: &ProjektInfo,
    parent_dir: Option<String>,
    splitflaechen: &[AenderungenIntersection],
    split_nas: &SplitNasXml,
    nas_xml: &NasXMLFile,
    extent_rect: Option<Rect>,
) {
    let legende = generate_legende_xlsx(splitflaechen);
    files.push((parent_dir.clone(), format!("Legende_{}.xlsx", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), legende));

    let header = generate_header_pdf(info, split_nas, extent_rect);
    files.push((parent_dir.clone(), format!("Blattkopf_{}.pdf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), header));

    let aenderungen_texte_bleibt = splitflaechen
        .iter().filter_map(|sf| sf.get_text_bleibt()).collect::<Vec<_>>();
    files.push((parent_dir.clone(), format!("Bleibt_Texte_{}.dxf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), texte_zu_dxf_datei(&aenderungen_texte_bleibt)));

    let aenderungen_texte_alt = splitflaechen
        .iter().filter_map(|sf| sf.get_text_alt()).collect::<Vec<_>>();
    files.push((parent_dir.clone(), format!("Alt_Texte_{}.dxf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), texte_zu_dxf_datei(&aenderungen_texte_alt)));

    let aenderungen_texte_neu = splitflaechen
        .iter().filter_map(|sf| sf.get_text_neu()).collect::<Vec<_>>();
    files.push((parent_dir.clone(), format!("Neu_Texte_{}.dxf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), texte_zu_dxf_datei(&aenderungen_texte_neu)));

    let aenderungen_rote_linien = get_aenderungen_rote_linien(&splitflaechen, nas_xml, split_nas);
    if !aenderungen_rote_linien.is_empty() {
        append_shp(files, &format!("Rote_Linien_{}", parent_dir.as_deref().unwrap_or("Aenderungen")), parent_dir.clone(), lines_to_shp(&aenderungen_rote_linien));
    }
    
    let aenderungen_nutzungsarten_linien = get_aenderungen_nutzungsarten_linien(&splitflaechen, nas_xml, split_nas);
    if !aenderungen_nutzungsarten_linien.is_empty() {
        append_shp(files, &format!("GeXte_Linien_{}", parent_dir.as_deref().unwrap_or("Aenderungen")), parent_dir.clone(), lines_to_shp(&aenderungen_nutzungsarten_linien));
    }
}

pub fn get_aenderungen_rote_linien(splitflaechen: &[AenderungenIntersection], nas: &NasXMLFile, split_nas: &SplitNasXml) -> Vec<SvgLine> {
    // let all_lines = splitflaechen.iter().flat_map(|s| s.poly_cut.)
    // TODO!
    splitflaechen.iter().flat_map(|s| {
        let mut lines = s.poly_cut.outer_rings.clone();
        lines.extend(s.poly_cut.inner_rings.iter().cloned());
        lines
    }).collect()
}

pub fn get_aenderungen_nutzungsarten_linien(splitflaechen: &[AenderungenIntersection], nas: &NasXMLFile, split_nas: &SplitNasXml) -> Vec<SvgLine> {
    Vec::new()
}

pub fn generate_anschlussriss_pdf(num: usize, total: usize, vert: bool) -> Vec<u8> {

    let text = format!("s. Anschlussriss ({num} / {total})");

    let (w, h) = match vert {
        true => (10.0, 50.0),
        false => (50.0, 10.0),
    };

    let (mut doc, page1, layer1) = PdfDocument::new(
        &text,
        Mm(w),
        Mm(h),
        &text,
    );
    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        .. Default::default()
    }));
    let helvetica = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
    let page1 = doc.get_page(page1);
    let layer1 = page1.get_layer(layer1);
    layer1.set_text_cursor(Mm(10.0), Mm(10.0));
    if vert {
        layer1.set_ctm(printpdf::CurTransMat::Rotate(270.0));
    }
    layer1.write_text(
        &text,
        &helvetica,
    );
    doc.save_to_bytes().unwrap_or_default()

}

pub fn generate_header_pdf(
    info: &ProjektInfo,
    split_nas: &SplitNasXml,
    extent_rect: Option<Rect>,
) -> Vec<u8> {

    let target_gemarkung_nr = info.gemarkung_nr.parse::<usize>().unwrap_or(0);

    let fluren_overlaps = split_nas.flurstuecke_nutzungen
    .values()
    .flat_map(|flst| flst.iter())
    .filter(|tp| match extent_rect {
        None => true,
        Some(s) => tp.get_rect().overlaps_rect(&s),
    })
    .filter_map(|s| {
        let flst = s.attributes.get("AX_Flurstueck")?;
        let flst_id = FlstIdParsed::from_str(&flst);
        if flst_id.gemarkung.parse::<usize>().ok()? != target_gemarkung_nr {
            None
        } else {
            flst_id.flur.parse::<usize>().ok()
        }
    })
    .collect::<BTreeSet<_>>();

    let (mut doc, page1, layer1) = PdfDocument::new(
        "Risskopf",
        Mm(175.0),
        Mm(35.0),
        "Risskopf Ebene 1",
    );

    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        .. Default::default()
    }));

    let helvetica = doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap();
    let page1 = doc.get_page(page1);
    let layer1 = page1.get_layer(layer1);
    layer1.set_text_cursor(Mm(10.0), Mm(10.0));
    layer1.write_text(
        format!("Flur {}", fluren_overlaps.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")),
        &helvetica,
    );
    doc.save_to_bytes().unwrap_or_default()
}

pub fn generate_legende_xlsx(
    splitflaechen: &[AenderungenIntersection]
) -> Vec<u8> {

    use simple_excel_writer::*;

    let alle_kuerzel = splitflaechen
    .iter()
    .flat_map(|s| vec![s.alt.clone(), s.neu.clone()].into_iter())
    .collect::<BTreeSet<_>>();

    let map: BTreeMap<String, NutzungsArt> = include!(concat!(env!("OUT_DIR"), "/nutzung.rs"));

    let mut lines = alle_kuerzel.iter().filter_map(|k| {
        let bez = &map.get(k)?.bez;
        Some(format!("{bez} ({k})"))
    }).collect::<Vec<_>>();

    lines.sort();

    let mut wb = Workbook::create_in_memory();
    let mut sheet = wb.create_sheet("Legende");

    // ID
    sheet.add_column(Column { width: 60.0 });


    let _ = wb.write_sheet(&mut sheet, |sheet_writer| {
        
        let sw = sheet_writer;
        sw.append_row(row!["Legende"])?;

        for l in lines.iter() {
            sw.append_row(row![l.to_string()])?;
        }

        Ok(())
    });

    match wb.close() {
        Ok(Some(o)) => o,
        _ => Vec::new(),
    }

}

pub fn append_shp(
    files: &mut Vec<(Option<String>, PathBuf, Vec<u8>)>,
    name: &str,
    parent_dir: Option<String>,
    shp_file: ShpReturn,
) {
    files.push((parent_dir.clone(), format!("{name}.shp").into(), shp_file.shp));
    files.push((parent_dir.clone(), format!("{name}.shx").into(), shp_file.shx));
    files.push((parent_dir.clone(), format!("{name}.dbf").into(), shp_file.dbf));
    files.push((parent_dir.clone(), format!("{name}.prj").into(), shp_file.prj));
    files.push((parent_dir.clone(), format!("{name}.cpg").into(), shp_file.cpg));
}