use std::{collections::{BTreeMap, BTreeSet}, io::BufWriter, path::PathBuf};

use dxf::{Header, Vector, XData, XDataItem};
use geo::Relate;
use printpdf::{BuiltinFont, CustomPdfConformance, IndirectFontRef, Mm, PdfConformance, PdfDocument, PdfLayerReference, Pt, Rgb};
use quadtree_f32::Rect;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use crate::{csv::{self, CsvDataType}, nas::{only_touches_internal, reproject_poly, translate_to_geo_poly, SvgPolygon, TaggedPolygon, UseRadians}, optimize::OptimizeConfig, pdf::{get_fluren, get_flurstuecke, get_gebaeude, get_mini_nas_xml, reproject_poly_back_into_latlon, RissConfig, RissExtent, RissExtentReprojected}, ui::{AenderungenIntersections, TextStatus}, uuid_wasm::log_status, xlsx::FlstIdParsedNumber};
use crate::{csv::CsvDatensatz, nas::{NasXMLFile, SplitNasXml, SvgLine, SvgPoint, LATLON_STRING}, pdf::{reproject_aenderungen_into_target_space, Konfiguration, ProjektInfo, Risse}, search::NutzungsArt, ui::{Aenderungen, AenderungenClean, AenderungenIntersection, TextPlacement}, xlsx::FlstIdParsed, zip::write_files_to_zip};
use serde_derive::{Serialize, Deserialize};

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
    csv_data: &CsvDataType,
) -> Vec<u8> {

    let mut files = Vec::new();

    let aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &split_nas.crs) {
        Ok(o) => o,
        Err(e) => return Vec::new(),
    };

    // RISSE -> risse.shp
    if !risse.is_empty() {
        append_shp(&mut files, "RISSE", None, generate_risse_shp(&risse, &split_nas.crs));
    }

    // Anschlussrisse PDFs
    let len = risse.len();
    for (i, _) in risse.iter().enumerate() {
        let i = i + 1;
        let pdf_horz = generate_anschlussriss_pdf(i, len, false);
        let pdf_vert = generate_anschlussriss_pdf(i, len, true);
        files.push((Some("Anschlussrisse".to_string()), format!("HORZ_{i}_von_{len}.pdf").into(), pdf_horz));
        files.push((Some("Anschlussrisse".to_string()), format!("VERT_{i}_von_{len}.pdf").into(), pdf_vert));
    }

    log_status("Berechne Splitflächen...");
    let splitflaechen = calc_splitflaechen(&aenderungen, split_nas, nas_xml, &csv_data);

    log_status(&format!("OK: {} Splitflächen", splitflaechen.0.len()));

    let splitflaechen_report = splitflaechen_zu_xlsx(csv_data, &splitflaechen);
    let mut antragsnr = projekt_info.antragsnr.trim().to_string();
    if antragsnr.is_empty() {
        antragsnr = "Aenderungen".to_string();
    }
    files.push((None, format!("{antragsnr}.Splitflaechen.xlsx").into(), splitflaechen_report));

    log_status(&format!("OK: {} Splitflächen exportiert in XLSX", splitflaechen.0.len()));

    let (num_eigentuemer, eigentuemer_xlsx) = eigentuemer_bearbeitete_flst_xlsx(csv_data, &splitflaechen);
    files.push((None, format!("{antragsnr}.EigentuemerBearbeiteteFlst.xlsx").into(), eigentuemer_xlsx));

    log_status(&format!("OK: {num_eigentuemer} Eigentümer exportiert in XLSX"));

    let lq = nas_xml.get_linien_quadtree();

    if risse.is_empty() {
        export_splitflaechen(
            &mut files, 
            projekt_info, 
            &csv_data,
            konfiguration,
            None, 
            &splitflaechen.0, 
            &split_nas, 
            &nas_xml,
            None,
            1,
            1,
            &lq,
        );
    } else {
        for (i, (_, r)) in risse.iter().enumerate() {
            export_splitflaechen(
                &mut files, 
                projekt_info, 
                &csv_data,
                konfiguration,
                Some(format!("Riss{}", i + 1)), 
                &splitflaechen.0, 
                &split_nas, 
                &nas_xml,
                Some(r.clone()),
                i + 1,
                risse.len(),
                &lq,
            );
        }
    }

    write_files_to_zip(&files)
}

pub fn eigentuemer_bearbeitete_flst_xlsx(
    datensaetze: &CsvDataType,
    splitflaechen: &AenderungenIntersections,
) -> (usize, Vec<u8>) {
    let data = datensaetze.iter().map(|(flst_id, v)| {
        (flst_id.clone(), v.iter().map(|cs| {
            CsvDatensatz {
                eigentuemer: cs.eigentuemer.trim().to_string(),
                nutzung: cs.nutzung.trim().to_string(),
                notiz: String::new(),
                status: AenderungenIntersection::get_auto_status(&splitflaechen.0, &flst_id),
            }
        }).collect())
    }).collect();

    crate::xlsx::flst_id_nach_eigentuemer(&data)
}

pub fn splitflaechen_zu_xlsx(
    datensaetze: &CsvDataType,
    splitflaechen: &AenderungenIntersections,
) -> Vec<u8> {
    
    use simple_excel_writer::*;
    
    let mut wb = Workbook::create_in_memory();
    let mut sheet = wb.create_sheet("Flurstuecke");

    // ID
    sheet.add_column(Column { width: 30.0 });
    // Nutzung
    sheet.add_column(Column { width: 60.0 });
    // Status
    sheet.add_column(Column { width: 30.0 });
    // Eigentümer
    sheet.add_column(Column { width: 60.0 });

    let _ = wb.write_sheet(&mut sheet, |sheet_writer| {
        let sw = sheet_writer;
        sw.append_row(row!["ID", "Nutzung", "Status", "Eigentümer"])?;
        for (flst_id, ds) in datensaetze.iter() {
            let ds_0 = match ds.get(0) {
                Some(s) => s,
                None => continue
            };
            let f = FlstIdParsed::from_str(flst_id).parse_num().unwrap_or_default();
            let notiz = AenderungenIntersection::get_auto_notiz(&splitflaechen.0, &flst_id);
            let status = AenderungenIntersection::get_auto_status(&splitflaechen.0, &flst_id);
            let mut eigentuemer = ds.iter().map(|s| s.eigentuemer.clone()).collect::<Vec<_>>();
            eigentuemer.sort();
            eigentuemer.dedup();
            let eig: String = eigentuemer.join("; ");
            let nutzung = ds_0.nutzung.clone();
            sw.append_row(row![
                FlstIdParsed::from_str(&flst_id).to_nice_string(),
                nutzung.to_string(),
                match status {
                    crate::csv::Status::Bleibt => "bleibt".to_string(),
                    crate::csv::Status::AenderungKeineBenachrichtigung => notiz + " (keine Benachrichtigung)",
                    crate::csv::Status::AenderungMitBenachrichtigung => notiz + " (mit Benachrichtigung)",
                },
                eig.to_string()
            ])?;
        }

        Ok(())
    });

    match wb.close() {
        Ok(Some(o)) => o,
        _ => Vec::new(),
    }
}

pub fn calc_splitflaechen(
    aenderungen: &Aenderungen,
    split_nas: &SplitNasXml,
    original_xml: &NasXMLFile,
    csv: &CsvDataType,
) -> AenderungenIntersections {

    let qt = split_nas.create_quadtree();

    let aenderungen = AenderungenClean {
        nas_xml_quadtree: qt,
        aenderungen: aenderungen.clone(),
    };

    log_status(&format!("Verschneide Änderungen.."));

    aenderungen.get_aenderungen_intersections(original_xml, crate::get_main_gemarkung(csv))
}

pub fn generate_risse_shp(
    riss_map: &Risse,
    target_crs: &str,
) -> ShpReturn {
    lines_to_shp(&riss_map.iter().filter_map(|(id, re)| {
        let re = re.get_extent(target_crs, 0.0)?;
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
    csv: &CsvDataType,
    konfiguration: &Konfiguration,
    parent_dir: Option<String>,
    splitflaechen: &[AenderungenIntersection],
    split_nas: &SplitNasXml,
    nas_xml: &NasXMLFile,
    riss: Option<RissConfig>,
    num_riss: usize,
    total_risse: usize,
    lq: &LinienQuadTree,
) {

    let mut default_riss_extent_rect = match splitflaechen.first() {
        Some(s) => s.poly_cut.get_rect(),
        None => return,
    };
    for sf in splitflaechen.iter().skip(1) {
        default_riss_extent_rect = default_riss_extent_rect.union(&sf.poly_cut.get_rect());
    }
    let utm_center = default_riss_extent_rect.get_center();
    let latlon_center = crate::pdf::reproject_point_back_into_latlon(&SvgPoint {
        x: utm_center.x,
        y: utm_center.y,
    }, &split_nas.crs).unwrap_or_default();

    let scale = 3500.0;
    let padding = 16.5 * 2.0;
    let default_riss_config = RissConfig {
        lat: latlon_center.y,
        lon: latlon_center.x,
        crs: "latlon".to_string(),
        width_mm: (default_riss_extent_rect.get_width() / scale * 1000.0).round() as f32 + padding + 10.0,
        height_mm: (default_riss_extent_rect.get_height() / scale * 1000.0).round() as f32 + padding + 10.0,
        scale: scale as f32,
        rissgebiet: None,
    };

    let riss = riss.clone().unwrap_or(default_riss_config);

    let riss_extent = match riss.get_extent(&split_nas.crs, 0.0) {
        Some(s) => s,
        None => return,
    };

    let riss_extent_with_border = match riss.get_extent(&split_nas.crs, padding.into()) {
        Some(s) => s,
        None => return,
    };

    let riss_extent_reprojected = match riss_extent.reproject(&split_nas.crs, &mut Vec::new()) {
        Some(s) => s,
        None => return,
    };

    let riss_extent_with_border_reprojected = match riss_extent_with_border.reproject(&split_nas.crs, &mut Vec::new()) {
        Some(s) => s,
        None => return,
    };

    let riss_extent_cutpoly_noborder = riss_extent_reprojected.get_poly();

    let riss_extent_cutpoly_withborder = riss_extent_with_border_reprojected.get_poly();

    let calc_pdf_preview = HeaderCalcConfig::from_csv(&split_nas, csv, &Some(riss_extent_cutpoly_withborder.clone()));

    let calc_pdf_final = HeaderCalcConfig::from_csv(&split_nas, csv, &Some(riss_extent_cutpoly_noborder.clone()));

    log_status(&format!("[{num_riss} / {total_risse}] Export {} Teilflächen", splitflaechen.len()));

    let header = generate_header_pdf(info, &calc_pdf_final, split_nas, num_riss, total_risse);
    files.push((parent_dir.clone(), format!("Blattkopf_{}.pdf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), header));

    let legende = generate_legende_xlsx(splitflaechen);
    files.push((parent_dir.clone(), format!("Legende_{}.xlsx", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), legende));

    let na_splitflaechen = get_na_splitflaechen(&splitflaechen, &split_nas, Some(riss_extent_reprojected.get_rect()));
    let aenderungen_nutzungsarten_linien = get_aenderungen_nutzungsarten_linien(&na_splitflaechen, lq);
    if !aenderungen_nutzungsarten_linien.is_empty() {
        append_shp(files, &format!("Linien_NAGrenze_Untergehend_{}", parent_dir.as_deref().unwrap_or("Aenderungen")), parent_dir.clone(), lines_to_shp(&aenderungen_nutzungsarten_linien));
    }
    log_status(&format!("{} Linien für untergehende NA-Grenzen generiert.", aenderungen_nutzungsarten_linien.len()));

    let aenderungen_rote_linien = get_aenderungen_rote_linien(&splitflaechen, lq);
    if !aenderungen_rote_linien.is_empty() {
        append_shp(files, &format!("Linien_Rot_{}", parent_dir.as_deref().unwrap_or("Aenderungen")), parent_dir.clone(), lines_to_shp(&aenderungen_rote_linien));
    }
    log_status(&format!("{} rote Linien generiert.", aenderungen_rote_linien.len()));

    let aenderungen_texte: Vec<TextPlacement> = AenderungenIntersections::get_texte(splitflaechen);
    log_status(&format!("{} Texte generiert", aenderungen_texte.len()));

    let aenderungen_texte_bleibt = aenderungen_texte
        .iter().filter(|sf| sf.status == TextStatus::StaysAsIs)
        .cloned().collect::<Vec<_>>();
    files.push((parent_dir.clone(), format!("Texte_Bleibt_{}.dxf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), texte_zu_dxf_datei(&aenderungen_texte_bleibt)));
    log_status(&format!("{} Texte: bleibende Kürzel", aenderungen_texte_bleibt.len()));

    let aenderungen_texte_alt = aenderungen_texte
        .iter().filter(|sf| sf.status == TextStatus::Old)
        .cloned().collect::<Vec<_>>();
    files.push((parent_dir.clone(), format!("Texte_Alt_{}.dxf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), texte_zu_dxf_datei(&aenderungen_texte_alt)));
    log_status(&format!("{} Texte: alte Kürzel", aenderungen_texte_alt.len()));

    let aenderungen_texte_neu = aenderungen_texte
        .iter().filter(|sf| sf.status == TextStatus::New)
        .cloned().collect::<Vec<_>>();
    files.push((parent_dir.clone(), format!("Texte_Neu_{}.dxf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), texte_zu_dxf_datei(&aenderungen_texte_neu)));
    log_status(&format!("{} Texte: neue Kürzel", aenderungen_texte_neu.len()));
    
    log_status(&format!("Generiere PDF-Vorschau..."));
    let mini_split_nas = get_mini_nas_xml(split_nas, &riss_extent_reprojected);
    let flst = get_flurstuecke(nas_xml, &riss_extent_reprojected);
    let fluren = get_fluren(nas_xml, &Some(riss_extent_reprojected.get_rect()));
    let gebaeude = get_gebaeude(nas_xml, &riss_extent_reprojected);

    let pdf_vorschau = crate::pdf::generate_pdf_internal(
        (num_riss, total_risse),
        info,
        &calc_pdf_preview,
        konfiguration,
        split_nas,
        &riss,
        &riss_extent_reprojected,
        // TODO: riss_extent_reprojected_noborder
        splitflaechen,
        &aenderungen_rote_linien,
        &aenderungen_nutzungsarten_linien,
        &aenderungen_texte,
        &fluren,
        &flst,
        &mini_split_nas,
        &gebaeude,
    );

    files.push((parent_dir.clone(), format!("Vorschau_{}.pdf", parent_dir.as_deref().unwrap_or("Aenderungen")).into(), pdf_vorschau));  
    log_status(&format!("PDF-Vorschau generiert."));
  
}

pub struct LinienQuadTree {
    pub linien: Vec<(SvgPoint, SvgPoint)>,
    pub qt: quadtree_f32::QuadTree,
}

impl LinienQuadTree {
    pub fn new(linien: Vec<(SvgPoint, SvgPoint)>) -> Self {
        
        let items = linien.iter().enumerate().filter_map(|(id, (a, b))| {
            let a = a.round_to_3dec();
            let b = b.round_to_3dec();
            if a.equals(&b) {
                return None;
            }
            Some((id, points_to_rect(&(a, b))))
        }).collect::<Vec<_>>();

        let max_items = items.len().saturating_div(20).max(500);

        let qt = quadtree_f32::QuadTree::new_with_max_items_per_quad(
            items.iter().map(|(k, v)| (quadtree_f32::ItemId(*k),  quadtree_f32::Item::Rect(*v))),
            max_items
        );
        
        Self {
            linien,
            qt,
        }
    }

    fn line_overlaps_or_equals(&self, a: &SvgPoint, b: &SvgPoint) -> bool {
        let rect = points_to_rect(&(*a, *b));
        let items = self.qt.get_ids_that_overlap(&rect).into_iter().filter_map(|i| self.linien.get(i.0)).cloned().collect::<Vec<_>>();
        for (s, t) in items.iter() {
            let a_on_line = a.equals(s) || a.equals(t) || crate::ui::dist_to_segment(*a, *s, *t).distance < 0.1;
            let b_on_line = b.equals(s) || b.equals(t) || crate::ui::dist_to_segment(*b, *s, *t).distance < 0.1;
            if a_on_line && b_on_line {
                return true;
            }
        }
        false
    }
}

pub fn l_to_points(l: &SvgLine) -> Vec<(SvgPoint, SvgPoint)> {
    let mut v = Vec::new();
    for p in l.points.windows(2) {
        match &p {
            &[a, b] => v.push((*a, *b)),
            _ => { },
        }
    }
    v
}

pub fn points_to_rect((a, b): &(SvgPoint, SvgPoint)) -> quadtree_f32::Rect {
    let max_x = a.x.max(b.x);
    let min_x = a.x.min(b.x);
    let max_y = a.y.max(b.y);
    let min_y = a.y.min(b.y);
    quadtree_f32::Rect {
        max_x, max_y, min_x, min_y
    }
}

pub fn get_aenderungen_rote_linien(
    splitflaechen: &[AenderungenIntersection], 
    linienquadtree: &LinienQuadTree
) -> Vec<SvgLine> {
    
    // rote linie: neue linie, die nicht auf nas xml linie liegt (oder teil einer nas xml linie ist)
    // 
    // -> create btree
    // -> select linien 
    // -> check if overlaps linie
    // -> deduplicate + join ends

    let mut alle_linien_zu_checken = splitflaechen.iter().flat_map(|s| {
        let mut lines = s.poly_cut.outer_rings.iter().flat_map(l_to_points).collect::<Vec<_>>();
        lines.extend(s.poly_cut.inner_rings.iter().flat_map(l_to_points));
        lines
    }).collect::<Vec<_>>();
    alle_linien_zu_checken.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    alle_linien_zu_checken.dedup();
    let alle_linien_zu_checken = alle_linien_zu_checken;

    let mut lines_end = Vec::new();
    for (ul_start, ul_end) in alle_linien_zu_checken.iter() {
        if !linienquadtree.line_overlaps_or_equals(&ul_start, &ul_end) {
            lines_end.push((*ul_start, *ul_end));
        }
    }
    lines_end.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    lines_end.dedup();

    lines_end.iter().map(|s| SvgLine {
        points: vec![s.0, s.1]
    }).collect()

    // TODO: buggy!
    // merge_lines_again(lines_end)
}

fn merge_lines_again(l: Vec<(SvgPoint, SvgPoint)>) -> Vec<SvgLine> {

    let mut v = l.into_iter().map(|(a, b)| vec![(a, b)]).collect::<Vec<_>>();

    loop {

        let mut modified_mark_remove = BTreeSet::new();
        let v_clone = v.clone();
        'outer: for (i, q) in v.iter_mut().enumerate() {

            if modified_mark_remove.contains(&i) {
                continue;
            }

            let first = match q.first().map(|s| s.0) {
                Some(s) => s,
                None => {
                    modified_mark_remove.insert(i);
                    continue;
                },
            };


            let last = match q.last().map(|s| s.1) {
                Some(s) => s,
                None => {
                    modified_mark_remove.insert(i);
                    continue;
                },
            };

            for (p, k) in v_clone.iter().enumerate() {

                if modified_mark_remove.contains(&p) || p == i {
                    continue;
                }

                let k_first = match k.first().map(|s| s.0) {
                    Some(s) => s,
                    None => {
                        modified_mark_remove.insert(p);
                        continue;
                    },
                };
    
                let k_last = match k.last().map(|s| s.1) {
                    Some(s) => s,
                    None => {
                        modified_mark_remove.insert(p);
                        continue;
                    },
                };

                let eps = 1.0;
                if last.equals_approx(&k_first, eps) {
                    let mut k_clone = k.clone();
                    q.append(&mut k_clone);
                    modified_mark_remove.insert(p);
                    break 'outer;
                } else if last.equals_approx(&k_last, 1.0) {
                    let mut k_clone = k.clone();
                    k_clone.reverse();
                    q.append(&mut k_clone);
                    modified_mark_remove.insert(p);
                    break 'outer;
                } else if first.equals_approx(&k_first, 1.0) {
                    let mut k_clone = k.clone();
                    q.reverse();
                    q.append(&mut k_clone);
                    q.reverse();
                    modified_mark_remove.insert(p);
                    break 'outer;
                } else if first.equals_approx(&k_last, 1.0) {
                    let mut k_clone = k.clone();
                    k_clone.reverse();
                    q.reverse();
                    q.append(&mut k_clone);
                    q.reverse();
                    modified_mark_remove.insert(p);
                    break 'outer;
                }
            }
        }

        if modified_mark_remove.is_empty() {
            break;
        }

        if modified_mark_remove.len() > v.len() {
            break; // error
        }

        let vlen = v.len();
        for (i, p) in modified_mark_remove.iter().enumerate() {
            v.swap(*p, vlen.saturating_sub(1).saturating_sub(i));
        }
        for _ in 0..modified_mark_remove.len() {
            v.pop();
        }
    }

    v.into_iter().filter_map(|p| {
        let mut points = p.into_iter().flat_map(|(a, b)| vec![a, b]).collect::<Vec<_>>();
        points.dedup_by(|a, b| a.equals(b));
        if points.is_empty() {
            None
        } else {
            Some(SvgLine { points })
        }
    }).collect::<Vec<_>>()
}

pub fn get_na_splitflaechen(splitflaechen: &[AenderungenIntersection], split_nas: &SplitNasXml, rect: Option<quadtree_f32::Rect>) -> Vec<AenderungenIntersection> {
    let mut finalized = splitflaechen.to_vec();
    let existing_flst = splitflaechen.iter().map(|f| &f.flst_id).collect::<BTreeSet<_>>();
    for (k, v) in split_nas.flurstuecke_nutzungen.iter() {
        finalized.extend(v.iter().filter_map(|q| {

            if let Some(r) = rect.as_ref() {
                if !q.poly.get_rect().overlaps_rect(r) {
                    return None;
                }
            }

            let flst_id = q.attributes.get("AX_Flurstueck")?;
            if existing_flst.contains(flst_id) {
                return None;
            }

            let ebene = q.attributes.get("AX_Ebene")?;
            let obj_id = q.attributes.get("id")?;
            let alt_kuerzel = q.get_auto_kuerzel(&ebene)?;
            let intersect_id = q.attributes
            .get("AX_IntersectionId")
            .map(|w| format!(":{w}"))
            .unwrap_or_default();
            let flst_id_part = format!("{flst_id}:{ebene}:{obj_id}{intersect_id}");

            Some(AenderungenIntersection {
                alt: alt_kuerzel.clone(),
                neu: alt_kuerzel,
                flst_id: flst_id.to_string(),
                flst_id_part: flst_id_part,
                poly_cut: q.poly.clone(),
            })
        }));
    }
    finalized
}

pub fn get_aenderungen_nutzungsarten_linien(splitflaechen: &[AenderungenIntersection], lq: &LinienQuadTree) -> Vec<SvgLine> {
    let mut pairs = BTreeSet::new();
    for (id1, s1) in splitflaechen.iter().enumerate() {

        let rect = s1.poly_cut.get_rect();
        let it = splitflaechen.iter().enumerate()
        .filter_map(|(i, p)| if p.poly_cut.get_rect().overlaps_rect(&rect) { Some((i, p)) } else { None });

        for (id2, s2) in it {
            if id1 == id2 {
                continue;
            }
            let hi = id1.max(id2);
            let lo = id1.min(id2);
            let pair = (lo, hi);
            if pairs.contains(&pair) {
                continue;
            }
            // Areas used to have distinct kuerzel, now they don't
            let should_insert = s1.alt != s2.alt && s1.neu == s2.neu;
            if !should_insert {
                continue;
            }

            pairs.insert(pair);
        }
    }

    let mut v = Vec::new();
    for (a, b) in pairs.iter() {
        let a = &splitflaechen[*a];
        let b = &splitflaechen[*b];
        let shared_lines = get_shared_lines(&a.poly_cut, &b.poly_cut);
        let mut shared_lines_2 = shared_lines.into_iter()
        .filter_map(|s| {
            let first = s.points.first()?;
            let last = s.points.last()?;
            if lq.line_overlaps_or_equals(first, last) {
                None
            } else {
                Some(s)
            }
        }).collect::<Vec<_>>();
        v.append(&mut shared_lines_2);
    }

    v
}

fn get_shared_lines(a: &SvgPolygon, b: &SvgPolygon) -> Vec<SvgLine> {
    let lines_a = get_linecoords(a);
    let lines_b = get_linecoords(b);
    let same = lines_a.intersection(&lines_b).collect::<Vec<_>>();

    let mut map = BTreeSet::new();
    for (start, end) in same {
        let (hi, lo) = if start.0 > end.0 {
            (start, end)
        } else {
            (end, start)
        };
        map.insert((hi, lo));
    }

    map.into_iter().map(|((ax, ay), (bx, by))| {
        SvgLine {
            points: vec![
                SvgPoint {
                    x: (*ax as f64) / 1000.0,
                    y: (*ay as f64) / 1000.0,
                },
                SvgPoint {
                    x: (*bx as f64) / 1000.0,
                    y: (*by as f64) / 1000.0,
                },
            ]
        }
    }).collect()
}

fn get_linecoords(p: &SvgPolygon) -> BTreeSet<((u64, u64), (u64, u64))> {
    let mut lines = p.outer_rings.iter().flat_map(crate::geograf::l_to_points).collect::<Vec<_>>();
    lines.extend(p.inner_rings.iter().flat_map(crate::geograf::l_to_points));
    lines.into_iter()
    .flat_map(|(a, b)| {
        vec![
            (((a.x * 1000.0) as u64, (a.y * 1000.0) as u64), ((b.x * 1000.0) as u64, (b.y * 1000.0) as u64)),
            (((b.x * 1000.0) as u64, (b.y * 1000.0) as u64), ((a.x * 1000.0) as u64, (a.y * 1000.0) as u64)),
        ]
    })
    .collect()
}

fn calc_text_width_pt(text: &String, font_scale: f32, font: &dyn ab_glyph::Font) -> Pt {
    // vertical scale of one text box
    let vert_scale = font.height_unscaled();

    // calculate the width of the text in unscaled units
    let sum_width: f32 = text
        .chars()
        .map(|ch| font.h_advance_unscaled(font.glyph_id(ch)))
        .sum();

    Pt(sum_width as f32 / (vert_scale as f32 / font_scale))
}

pub fn generate_anschlussriss_pdf(num: usize, total: usize, vert: bool) -> Vec<u8> {

    use ab_glyph::FontRef;

    let text = format!("s. Anschlussriss ({num} / {total})");

    let face = match FontRef::try_from_slice(crate::ARIAL_TTF) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let text_h = 3.0; // Mm
    let text_width: Mm = calc_text_width_pt(&text, Mm(text_h).into_pt().0, &face).into();
    let text_w = text_width.0 + 4.0; // Mm
    let padding = 1.0;

    let (w, h) = match vert {
        true => (text_h + padding + padding, text_w + padding + padding),
        false => (text_w + padding + padding, text_h + padding + padding),
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

    let arial = match doc.add_external_font(crate::ARIAL_TTF) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let page1 = doc.get_page(page1);
    let layer1 = page1.get_layer(layer1);

    layer1.set_outline_thickness(Mm(1.0).into_pt().0);
    layer1.set_outline_color(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let points = vec![
        (printpdf::Point { x: Mm(0.0).into(), y: Mm(0.0).into() }, false),
        (printpdf::Point { x: Mm(w).into(), y: Mm(0.0).into() }, false),
        (printpdf::Point { x: Mm(w).into(), y: Mm(h).into() }, false),
        (printpdf::Point { x: Mm(0.0).into(), y: Mm(h).into() }, false),
    ];

    let poly = printpdf::Polygon {
        rings: vec![points],
        mode: printpdf::path::PaintMode::Stroke,
        winding_order: printpdf::path::WindingOrder::NonZero,
    };

    layer1.add_polygon(poly);

    layer1.begin_text_section();
    layer1.set_font(&arial, Mm(text_h).into_pt().0);

    if vert {
        layer1.set_text_matrix(printpdf::TextMatrix::TranslateRotate(
            Mm(w - (padding * 1.5)).into_pt(), Mm(padding * 1.5).into_pt(), 90.0)
        );
    } else {
        layer1.set_text_cursor(Mm(padding * 1.5), Mm(padding * 1.5));
    }

    layer1.write_text(&text, &arial);
    layer1.end_text_section();

    doc.save_to_bytes().unwrap_or_default()
}

pub fn generate_header_pdf(
    info: &ProjektInfo,
    calc: &HeaderCalcConfig,
    split_nas: &SplitNasXml,
    num_riss: usize,
    total_risse: usize,
) -> Vec<u8> {

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

    let times_roman = match doc.add_builtin_font(BuiltinFont::TimesRoman) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let times_roman_bold = match doc.add_builtin_font(BuiltinFont::TimesBold) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let page1 = doc.get_page(page1);
    let mut layer1 = page1.get_layer(layer1);

    let _ = write_header(
        &mut layer1,
        info,
        calc,
        &times_roman,
        &times_roman_bold,
        num_riss,
        total_risse,
        0.0,
        0.0,
    );

    doc.save_to_bytes().unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderCalcConfig {
    pub gemarkungs_nr: usize,
    pub gemarkungen: BTreeMap<usize, BTreeMap<usize, Vec<FlstIdParsedNumber>>>,
}

impl HeaderCalcConfig {
    pub fn get_flst_string(&self) -> String {

        let default = BTreeMap::new();
        let f = self.gemarkungen.get(&self.gemarkungs_nr).unwrap_or(&default);
        if f.len() != 1 {
            "diverse".to_string()
        } else {
            let flst = f.iter().next().unwrap().1;
            let mut cl = flst.clone();
            cl.sort();
            cl.dedup();
            if cl.len() > 3 {
                "diverse".to_string()
            } else {
                cl.iter().map(|s| s.format_str()).collect::<Vec<_>>().join(", ").trim().to_string()
            }
        }
    }

    fn get_fluren_string_internal(&self) -> Vec<String> {
        let default = BTreeMap::new();
        let f = self.gemarkungen.get(&self.gemarkungs_nr).unwrap_or(&default);
        f.keys().map(|s| s.to_string()).collect::<Vec<_>>()
    }

    pub fn get_fluren_string(&self) -> String {
        let internal = self.get_fluren_string_internal();
        if internal.len() > 3 {
            "diverse".to_string()
        } else {
            internal.join(", ").trim().to_string()
        }
    }

    pub fn get_fluren_len(&self) -> usize {
        self.get_fluren_string_internal().len()
    }

    pub fn from_csv(split_nas: &SplitNasXml, csv: &CsvDataType, extent_poly: &Option<SvgPolygon>) -> Self {

        let target_gemarkung_nr = crate::get_main_gemarkung(csv);
        
        let mut flst_overlaps = Vec::new();
        match extent_poly.as_ref() {
            Some(p) => {
                let mut s = p.clone();
                s.correct_winding_order();

                let qt = split_nas.create_quadtree();
                
                flst_overlaps = 
                qt.get_overlapping_flst(&s.get_rect())
                .into_iter()
                .filter_map(|tp| {
                    let mut poly = tp.1.poly.clone();
                    poly.correct_winding_order();
                    if poly.is_inside_of(&s) {
                        Some(tp.1)
                    } else {
                        None
                    }
                })
                .collect();
            },
            None => {
                flst_overlaps = split_nas.flurstuecke_nutzungen.values().flat_map(|flst| flst.iter().cloned()).collect();
            }
        }

        let mut flst_overlaps = flst_overlaps.into_iter()
        .filter_map(|s| {
            let flst = s.attributes.get("AX_Flurstueck")?;
            let flst_id = FlstIdParsed::from_str(&flst).parse_num()?;
            Some(flst_id)
        })
        .collect::<Vec<_>>();

        flst_overlaps.sort();
        flst_overlaps.dedup();
        
        let mut gemarkungen = BTreeMap::new();
        for f in flst_overlaps {
            gemarkungen.entry(f.gemarkung)
            .or_insert_with(|| BTreeMap::new())
            .entry(f.flur)
            .or_insert_with(|| Vec::new())
            .push(f);
        }

        Self {
            gemarkungs_nr: target_gemarkung_nr,
            gemarkungen,
        }
    }
}


pub fn write_header(
    layer1: &mut PdfLayerReference,
    info: &ProjektInfo,
    calc: &HeaderCalcConfig,
    times_roman: &IndirectFontRef,
    times_roman_bold: &IndirectFontRef,
    num_riss: usize,
    total_risse: usize,
    offset_top: f32,
    offset_right: f32,
) -> Option<()> {

    layer1.save_graphics_state();

    let header_font_size = 14.0; // pt
    let medium_font_size = 10.0; // pt
    let small_font_size = 8.0; // pt

    layer1.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));

    let text = format!("Ergänzungsriss: Tatsächliche Nutzung ( {num_riss} / {total_risse} )");
    layer1.use_text(&text, header_font_size, Mm(offset_right + 2.0), Mm(offset_top + 30.0), &times_roman_bold);    

    let text = "Gemeinde:";
    layer1.use_text(text, small_font_size, Mm(offset_right + 2.0), Mm(offset_top + 25.0), &times_roman);    

    let text = "Gemarkung:";
    layer1.use_text(text, small_font_size, Mm(offset_right + 2.0), Mm(offset_top + 17.0), &times_roman);    

    let text = "Flur";
    layer1.use_text(text, small_font_size, Mm(offset_right + 2.0), Mm(offset_top + 10.0), &times_roman);    

    let text = "Instrument/Nr.";
    layer1.use_text(text, small_font_size, Mm(offset_right + 2.0), Mm(offset_top + 3.0), &times_roman);    

    let text = "-";
    layer1.use_text(text, medium_font_size, Mm(offset_right + 32.0), Mm(offset_top + 2.0), &times_roman);    

    let text = "Flurstücke";
    layer1.use_text(text, small_font_size, Mm(offset_right + 20.0), Mm(offset_top + 10.0), &times_roman);    

    let text = "Bearbeitung beendet am:";
    layer1.use_text(text, small_font_size, Mm(offset_right + 62.0), Mm(offset_top + 25.0), &times_roman);    

    let text = format!("Erstellt durch: {} ({})", info.erstellt_durch, info.beruf_kuerzel);
    layer1.use_text(text, small_font_size, Mm(offset_right + 62.0), Mm(offset_top + 17.0), &times_roman);    

    let text = "Vermessungsstelle";
    layer1.use_text(text, small_font_size, Mm(offset_right + 62.0), Mm(offset_top + 10.0), &times_roman);    

    let text = "Grenztermin vom";
    layer1.use_text(text, small_font_size, Mm(offset_right + 104.0), Mm(offset_top + 25.0), &times_roman);    

    let text = "-";
    layer1.use_text(text, medium_font_size, Mm(offset_right + 120.0), Mm(offset_top + 21.0), &times_roman);    

    let text = "Verwendete Vermessungsun-";
    layer1.use_text(text, small_font_size, Mm(offset_right + 104.0), Mm(offset_top + 17.0), &times_roman);    
    let text = "terlagen";
    layer1.use_text(text, small_font_size, Mm(offset_right + 104.0), Mm(offset_top + 14.0), &times_roman);    

    let text = format!("ALKIS ({})", info.alkis_aktualitaet);
    layer1.use_text(text, small_font_size, Mm(offset_right + 104.0), Mm(offset_top + 10.0), &times_roman);    
    let text = format!("Orthophoto ({})", info.orthofoto_datum);
    layer1.use_text(text, small_font_size, Mm(offset_right + 104.0), Mm(offset_top + 6.5), &times_roman);    
    let text = format!("GIS-Feldblöcke ({})", info.gis_feldbloecke_datum);
    layer1.use_text(text, small_font_size, Mm(offset_right + 104.0), Mm(offset_top + 3.0), &times_roman);    

    let text = "Archivblatt: *";
    layer1.use_text(text, small_font_size, Mm(offset_right + 140.0), Mm(offset_top + 21.0), &times_roman);    

    let text = "Antrags-Nr.: *";
    layer1.use_text(text, small_font_size, Mm(offset_right + 140.0), Mm(offset_top + 17.0), &times_roman);    

    let text = info.antragsnr.trim().to_string();
    layer1.use_text(text, small_font_size, Mm(offset_right + 140.0), Mm(offset_top + 14.0), &times_roman);    

    let text = "Katasteramt:";
    layer1.use_text(text, small_font_size, Mm(offset_right + 140.0), Mm(offset_top + 10.0), &times_roman);    

    let text = info.katasteramt.trim();
    layer1.use_text(text, medium_font_size, Mm(offset_right + 150.0), Mm(offset_top + 2.0), &times_roman);    

    let text = info.gemeinde.trim();
    layer1.use_text(text, medium_font_size, Mm(offset_right + 20.0), Mm(offset_top + 21.0), &times_roman);    

    let text = format!("{} ({})", info.gemarkung.trim(), calc.gemarkungs_nr);
    layer1.use_text(text, medium_font_size, Mm(offset_right + 20.0), Mm(offset_top + 14.0), &times_roman);    

    let text = calc.get_flst_string();
    layer1.use_text(text, medium_font_size, Mm(offset_right + 32.0), Mm(offset_top + 7.0), &times_roman);    

    let text = info.bearbeitung_beendet_am.trim();
    layer1.use_text(text, medium_font_size, Mm(offset_right + 73.0), Mm(offset_top + 21.0), &times_roman);    

    let text = info.vermessungsstelle.trim();
    layer1.use_text(text, medium_font_size, Mm(offset_right + 68.0), Mm(offset_top + 2.0), &times_roman);    

    let text = calc.get_fluren_string();
    let fluren_len = calc.get_fluren_len();
    let offset_right_fluren = match fluren_len {
        0 => offset_right + 8.0,
        1 => offset_right + 8.0,
        2 => offset_right + 6.0,
        3 => offset_right + 4.0,
        _ => offset_right + 8.0,
    };
    layer1.use_text(&text, medium_font_size, Mm(offset_right_fluren), Mm(offset_top + 7.0), &times_roman);    
    
    let lines = &[
        ((offset_right + 0.0, offset_top + 28.0), (offset_right + 139.0, offset_top + 28.0)),
        ((offset_right + 0.0, offset_top + 20.0), (offset_right + 175.0, offset_top + 20.0)),
        ((offset_right + 0.0, offset_top + 13.0), (offset_right + 102.0, offset_top + 13.0)),
        ((offset_right + 139.0, offset_top + 13.0), (offset_right + 175.0, offset_top + 13.0)),
        ((offset_right + 0.0, offset_top + 6.0), (offset_right + 60.0, offset_top + 6.0)),

        ((offset_right + 17.0, offset_top + 13.0), (offset_right + 17.0, offset_top + 6.0)),
        ((offset_right + 60.0, offset_top + 28.0), (offset_right + 60.0, offset_top + 0.0)),
        ((offset_right + 102.0, offset_top + 28.0), (offset_right + 102.0, offset_top + 0.0)),
        ((offset_right + 139.0, offset_top + 28.0), (offset_right + 139.0, offset_top + 0.0)),
    ];

    layer1.set_outline_thickness(0.5);
    layer1.set_outline_color(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    for ((x0, y0), (x1, y1)) in lines.iter() {
        layer1.add_line(printpdf::Line {
            points: vec![
                (printpdf::Point {
                    x: Mm(*x0).into(),
                    y: Mm(*y0).into(),
                }, false),
                (printpdf::Point {
                    x: Mm(*x1).into(),
                    y: Mm(*y1).into(),
                }, false),
            ],
            is_closed: false,
        })
    }

    layer1.restore_graphics_state();

    Some(())
}

pub fn generate_legende_xlsx(
    splitflaechen: &[AenderungenIntersection]
) -> Vec<u8> {

    use simple_excel_writer::*;

    let alle_kuerzel = splitflaechen
    .iter()
    .flat_map(|s| {
        vec![s.alt.clone(), s.neu.clone()].into_iter()
    })
    .collect::<BTreeSet<_>>();

    let map = crate::get_map();

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
        sw.append_row(row!["Legende Abkürzungen"])?;

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