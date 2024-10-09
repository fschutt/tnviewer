use crate::{
    csv::{
        CsvDataType,
        Status,
    },
    nas::{
        self,
        NasXMLFile,
        SplitNasXml,
        SvgLine,
        SvgPoint,
        SvgPolygonInner, TaggedPolygon,
    },
    optimize::{OptimizeConfig, OptimizedTextPlacement},
    pdf::{
        get_fluren, get_flurstuecke, get_gebaeude, get_mini_nas_xml, reproject_aenderungen_into_target_space, HintergrundCache, Konfiguration, ProjektInfo, RissConfig, RissExtentReprojected, Risse
    },
    process::{
        AngleDegrees,
        PointOnLineConfig,
    },
    ui::{
        Aenderungen, AenderungenClean, AenderungenIntersection, AenderungenIntersections, TextPlacement, TextStatus
    },
    uuid_wasm::{log_status, uuid},
    xlsx::{
        FlstIdParsed,
        FlstIdParsedNumber,
    },
    xml_templates::{
        AntragsbegleitblattInfo,
        BearbeitungslisteInfo,
        FortfuehrungsbelegInfo,
    },
    zip::write_files_to_zip,
};
use dxf::{entities::Text, Vector};
use printpdf::{
    BuiltinFont,
    CustomPdfConformance,
    Mm,
    PdfConformance,
    PdfDocument,
    Pt,
    Rgb,
};
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    io::BufWriter,
    path::PathBuf,
};

fn update_dxf_x(zone: usize, pos: f64) -> f64 {
    format!("{zone}{pos}").parse().unwrap_or_default()
}

/// Returns the dxf bytes
pub fn texte_zu_dxf_datei(texte: &[TextPlacement]) -> Vec<u8> {
    use dxf::{
        entities::*,
        Drawing,
    };

    let mut drawing = Drawing::new();
    let zone = 33;

    for text in texte {
        let newx = update_dxf_x(zone, text.pos.x);
        let location = dxf::Point {
            x: newx,
            y: text.pos.y,
            z: 0.0,
        };
        let entity = Entity::new(EntityType::Text(dxf::entities::Text {
            thickness: 0.0,
            location,
            text_height: 5.0,
            value: text.kuerzel.clone(),
            rotation: 0.0,
            relative_x_scale_factor: 1.0,
            oblique_angle: 0.0,
            text_style_name: match text.status {
                crate::ui::TextStatus::Old => "old",
                crate::ui::TextStatus::New => "new",
                crate::ui::TextStatus::StaysAsIs => "stayasis",
            }
            .to_string(),
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

pub fn lines_to_points(lines: &[SvgLine]) -> Vec<(SvgPoint, AngleDegrees)> {
    let line_points = lines
        .iter()
        .flat_map(|s| s.to_points_vec())
        .collect::<Vec<_>>();
    let lines_joined = merge_lines_again(line_points);
    let config = &PointOnLineConfig {
        symbol_width_m: 5.0,
        distance_on_line_m: 10.0,
    };
    crate::process::generate_points_along_lines(config, &lines_joined)
}

pub fn lines_to_points_dxf(lines: &[SvgLine]) -> Vec<u8> {
    use dxf::{
        entities::*,
        Drawing,
    };

    let points = lines_to_points(lines);

    let mut drawing = Drawing::new();
    let zone = 33;

    for (location, angle) in points.iter() {
        let newx = update_dxf_x(zone, location.x);
        let entity = Entity::new(EntityType::ModelPoint(ModelPoint {
            location: dxf::Point {
                x: newx,
                y: location.y,
                z: 0.0,
            },
            angle: *angle,
            ..Default::default()
        }));
        let _entity_ref = drawing.add_entity(entity);
    }

    let v = Vec::new();
    let mut buf = BufWriter::new(v);
    let _ = drawing.save(&mut buf);
    buf.into_inner().unwrap_or_default()
}

pub fn lines_to_dxf(lines: &[SvgLine]) -> Vec<u8> {
    use dxf::{
        entities::*,
        Drawing,
    };

    let mut drawing = Drawing::new();
    let zone = 33;

    for l in lines.iter() {
        let entity = Entity::new(EntityType::Polyline({
            let mut m = Polyline::default();
            for pos in l.points.iter() {
                let newx = update_dxf_x(zone, pos.x);
                let location = dxf::Point {
                    x: newx,
                    y: pos.y,
                    z: 0.0,
                };
                m.add_vertex(&mut drawing, Vertex::new(location));
            }
            m
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
    use shapefile::{
        dbase::Record,
        record::polyline::Polyline,
        Point,
    };
    use std::io::Cursor;

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

pub async fn export_aenderungen_geograf(
    split_nas: &SplitNasXml, // original projection
    nas_xml: &NasXMLFile,    // original projection
    projekt_info: &ProjektInfo,
    konfiguration: &Konfiguration,
    aenderungen: &Aenderungen,
    risse: &Risse,
    csv_data: &CsvDataType,
    render_hintergrund_vorschau: bool,
    use_dgm: bool,
) -> Vec<u8> {
    let mut files = Vec::new();

    let aenderungen = match reproject_aenderungen_into_target_space(&aenderungen, &split_nas.crs) {
        Ok(o) => o,
        Err(_e) => return Vec::new(),
    };

    let gebaeude_flst = aenderungen.get_gebaeude_modified_flst();

    let mut antragsnr = projekt_info.antragsnr.trim().to_string();
    if antragsnr.is_empty() {
        antragsnr = "Aenderungen".to_string();
    }

    // Anschlussrisse PDFs
    let len = risse.len();
    for (i, (_, rc)) in risse.iter().enumerate() {
        let i = i + 1;
        let pdf_horz = generate_anschlussriss_pdf(i, len, false);
        let pdf_vert = generate_anschlussriss_pdf(i, len, true);
        files.push((
            Some("Anschlussrisse".to_string()),
            format!("HORZ_{i}_von_{len}.pdf").into(),
            pdf_horz,
        ));
        files.push((
            Some("Anschlussrisse".to_string()),
            format!("VERT_{i}_von_{len}.pdf").into(),
            pdf_vert,
        ));

        let ex = rc
            .get_extent_special(&split_nas.crs)
            .and_then(|s| s.reproject(&split_nas.crs));

        let line = match ex {
            Some(s) => s.get_rect_line(),
            None => continue,
        };

        append_shp(
            &mut files,
            &format!("Riss{i}"),
            Some("Plotboxen".to_string()),
            lines_to_shp(&[line]),
        );
    }

    log_status("Berechne Splitflächen...");
    let splitflaechen = calc_splitflaechen(&aenderungen, split_nas, nas_xml, &csv_data);
    log_status(&format!("OK: {} Splitflächen", splitflaechen.0.len()));
    let main_gemarkung = crate::get_main_gemarkung(&csv_data);
    let eigentuemer_map = splitflaechen_eigentuemer_map(&csv_data, &splitflaechen, &gebaeude_flst);
    let splitflaechen_xlsx =
        crate::xml_templates::generate_bearbeitungsliste_xlsx(&BearbeitungslisteInfo {
            eigentuemer: eigentuemer_map.clone(),
            auftragsnr: projekt_info.antragsnr.trim().to_string(),
            gemarkung_name: projekt_info.gemarkung.clone(),
            fluren: get_fluren_string(&splitflaechen, main_gemarkung),
        });
    files.push((
        None,
        format!("{antragsnr}.Bearbeitungsliste.xlsx").into(),
        splitflaechen_xlsx,
    ));
    log_status(&format!(
        "OK: {} Flurstücke exportiert in Bearbeitungsliste",
        eigentuemer_map.len()
    ));

    let eigentuemer_map_modified =
        splitflaechen_eigentuemer_map_modified(&csv_data, &splitflaechen, &gebaeude_flst);
    let modified = get_modified_fluren_flst(&eigentuemer_map_modified, main_gemarkung);
    let antragsbegleitblatt =
        crate::xml_templates::generate_antragsbegleitblatt_docx(&AntragsbegleitblattInfo {
            datum: projekt_info.bearbeitung_beendet_am.clone(),
            antragsnr: projekt_info.antragsnr.replace("-30-", "-51-"),
            gemarkung: projekt_info.gemarkung.trim().to_string(),
            gemarkungsnummer: main_gemarkung.to_string(),
            fluren_bearbeitet: modified
                .keys()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            flurstuecke_bearbeitet: join_modified_fluren(&modified).into_iter().collect(),
            eigentuemer: get_eigentuemer(&eigentuemer_map_modified),
        });
    files.push((
        None,
        format!("{antragsnr}.Antragsbegleitblatt.docx").into(),
        antragsbegleitblatt,
    ));

    let splitflaechen_xlsx =
        crate::xml_templates::generate_fortfuehrungsbeleg_docx(&FortfuehrungsbelegInfo {
            datum: projekt_info.bearbeitung_beendet_am.clone(),
            jahrgang: projekt_info
                .antragsnr
                .split("-")
                .next()
                .unwrap_or("2024")
                .to_string(),
            gemeindename: projekt_info.gemeinde.clone(),
            gemarkungsname: projekt_info.gemarkung.clone(),
            gemarkungsnummer: main_gemarkung.to_string(),
            fluren_modified: join_fluren(&modified.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
            antragsnummer_51: projekt_info.antragsnr.replace("-30-", "-51-"),
            tatsaechliche_nutzung_modified: splitflaechen.0.iter().any(|s| s.alt != s.neu),
            topografie_und_bauwerke_modified: !aenderungen.gebaeude_loeschen.is_empty(),
        });
    files.push((
        None,
        format!("{antragsnr}.Fortfuehrungsbeleg.docx").into(),
        splitflaechen_xlsx,
    ));
    log_status(&format!("OK: Fortführungsbeleg erstellt"));

    let lq_flurstuecke = nas_xml.get_linien_quadtree();
    let lq_flurstuecke_und_nutzungsarten = split_nas.get_linien_quadtree();

    let risse2 = if render_hintergrund_vorschau {
        risse.iter().map(|s| s.1.clone()).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut hintergrund_cache = HintergrundCache::build(
        if use_dgm {
            konfiguration.map.dgm_source.clone()
        } else {
            konfiguration.map.dop_source.clone()
        },
        if use_dgm {
            konfiguration.map.dgm_layers.clone()
        } else {
            konfiguration.map.dop_layers.clone()
        },
        &risse2,
        &split_nas.crs,
    )
    .await;

    let gebaeude_ids = aenderungen.gebaeude_loeschen.values()
    .map(|s| s.gebaeude_id.clone())
    .collect::<BTreeSet<_>>();

    let ax_gebaeude = nas_xml.ebenen.get("AX_Gebaeude")
    .unwrap_or(&Vec::new())
    .iter().filter_map(|tp| {
        let obj_id = tp.attributes.get("id")?;
        if gebaeude_ids.contains(obj_id) {
            Some(tp.clone())
        } else {
            None
        }
    }).collect::<Vec<_>>();

    let mut grafbat_map = BTreeMap::new();
    if risse.is_empty() {
       if let Ok((id, s)) = export_splitflaechen(
            &mut files,
            projekt_info,
            &csv_data,
            konfiguration,
            None,
            &splitflaechen.0,
            &ax_gebaeude,
            &split_nas,
            &nas_xml,
            None,
            1,
            1,
            &lq_flurstuecke,
            &lq_flurstuecke_und_nutzungsarten,
            &mut hintergrund_cache,
        ) {
            grafbat_map.insert(id, s);
        }
    } else {
        for (i, (_, r)) in risse.iter().enumerate() {
            if let Ok((id, s)) = export_splitflaechen(
                &mut files,
                projekt_info,
                &csv_data,
                konfiguration,
                Some(format!("Riss{}", i + 1)),
                &splitflaechen.0,
                &ax_gebaeude,
                &split_nas,
                &nas_xml,
                Some(r.clone()),
                i + 1,
                risse.len(),
                &lq_flurstuecke,
                &lq_flurstuecke_und_nutzungsarten,
                &mut hintergrund_cache,
            ) {
                grafbat_map.insert(id, s);
            }
        }
    }

    if let Ok(default_extent) = get_default_riss_extent_2(&splitflaechen.0, &ax_gebaeude, split_nas) {
        let grafbat = generate_grafbat_out(
            projekt_info,
            &default_extent,
            grafbat_map,
        );
        files.push((None, format!("{}.GRAFBAT.out", projekt_info.antragsnr).into(), grafbat.as_bytes().to_vec()));
    }

    write_files_to_zip(files)
}

fn get_fluren_string(splitflaechen: &AenderungenIntersections, main_gemarkung: usize) -> String {
    let fl = splitflaechen
        .get_fluren(main_gemarkung)
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    format_fluren(&fl)
}

fn format_fluren(fl: &[String]) -> String {
    if fl.is_empty() {
        String::new()
    } else if fl.len() == 1 {
        fl[0].clone()
    } else {
        let first = fl.first().unwrap();
        let last = fl.last().unwrap();
        format!("{first} - {last}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlstEigentuemer {
    pub nutzung: String,
    pub status: Status,
    pub eigentuemer: Vec<EigentuemerClean>,
    pub notiz: String,
    pub auto_notiz: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EigentuemerClean {
    Herr { vorname: String, nachname: String },
    Frau { vorname: String, nachname: String },
    Firma { name: String },
    Sonstige { name: String },
}

impl PartialOrd for EigentuemerClean {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.order(other))
    }
}

impl Ord for EigentuemerClean {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order(other)
    }
}

impl EigentuemerClean {
    pub fn order(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (
                EigentuemerClean::Herr { vorname, nachname },
                EigentuemerClean::Frau {
                    vorname: _1,
                    nachname: _2,
                },
            )
            | (
                EigentuemerClean::Herr { vorname, nachname },
                EigentuemerClean::Herr {
                    vorname: _1,
                    nachname: _2,
                },
            )
            | (
                EigentuemerClean::Frau { vorname, nachname },
                EigentuemerClean::Frau {
                    vorname: _1,
                    nachname: _2,
                },
            )
            | (
                EigentuemerClean::Frau { vorname, nachname },
                EigentuemerClean::Herr {
                    vorname: _1,
                    nachname: _2,
                },
            ) => {
                let a = format!("{vorname} {nachname}");
                let b = format!("{_1} {_2}");
                a.cmp(&b)
            }
            (EigentuemerClean::Firma { name }, EigentuemerClean::Firma { name: name2 }) => {
                name.cmp(name2)
            }
            (EigentuemerClean::Sonstige { name }, EigentuemerClean::Sonstige { name: name2 }) => {
                name.cmp(name2)
            }
            (oa, ob) => oa.rank().cmp(&ob.rank()),
        }
    }

    pub fn rank(&self) -> usize {
        match self {
            EigentuemerClean::Firma { name: _ } => 0,
            EigentuemerClean::Sonstige { name: _ } => 1,
            EigentuemerClean::Herr { .. } | EigentuemerClean::Frau { .. } => 2,
        }
    }

    pub fn format(&self) -> String {
        match self {
            EigentuemerClean::Herr { vorname, nachname }
            | EigentuemerClean::Frau { vorname, nachname } => format!("{vorname} {nachname}"),
            EigentuemerClean::Firma { name } => name.trim().to_string(),
            EigentuemerClean::Sonstige { name } => name.trim().to_string(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        let comma_split = s
            .split(",")
            .map(|q| q.trim().to_string())
            .collect::<Vec<_>>();
        if comma_split.iter().any(|s| s.contains("Herr")) {
            let vorname = comma_split
                .get(1)
                .map(|s| s.replace("(Herr)", "").trim().to_string())
                .unwrap_or_default();
            let nachname = comma_split
                .get(0)
                .map(|s| s.replace("(Herr)", "").trim().to_string())
                .unwrap_or_default();
            Self::Herr { vorname, nachname }
        } else if comma_split.iter().any(|s| s.contains("Frau")) {
            let vorname = comma_split
                .get(1)
                .map(|s| s.replace("(Frau)", "").trim().to_string())
                .unwrap_or_default();
            let nachname = comma_split
                .get(0)
                .map(|s| s.replace("(Frau)", "").trim().to_string())
                .unwrap_or_default();
            Self::Herr { vorname, nachname }
        } else if comma_split.iter().any(|s| s.contains("Firma")) {
            Self::Firma {
                name: s.replace("(Firma)", "").trim().to_string(),
            }
        } else {
            Self::Sonstige {
                name: s.trim().to_string(),
            }
        }
    }
}

pub fn splitflaechen_eigentuemer_map(
    datensaetze: &CsvDataType,
    splitflaechen: &AenderungenIntersections,
    gebaeude_flst: &[FlstIdParsedNumber],
) -> BTreeMap<FlstIdParsedNumber, FlstEigentuemer> {
    splitflaechen_eigentuemer_map_internal(datensaetze, splitflaechen, gebaeude_flst, false)
}

pub fn splitflaechen_eigentuemer_map_modified(
    datensaetze: &CsvDataType,
    splitflaechen: &AenderungenIntersections,
    gebaeude_flst: &[FlstIdParsedNumber],
) -> BTreeMap<FlstIdParsedNumber, FlstEigentuemer> {
    splitflaechen_eigentuemer_map_internal(datensaetze, splitflaechen, gebaeude_flst, true)
}

fn splitflaechen_eigentuemer_map_internal(
    datensaetze: &CsvDataType,
    splitflaechen: &AenderungenIntersections,
    gebaeude_flst: &[FlstIdParsedNumber],
    filter_only_modified: bool,
) -> BTreeMap<FlstIdParsedNumber, FlstEigentuemer> {
    datensaetze
        .get_old_fallback()
        .iter()
        .filter_map(|(flst_id, ds)| {
            let ds_0 = ds.get(0)?;
            let f = FlstIdParsed::from_str(flst_id).parse_num()?;
            let notiz = AenderungenIntersection::get_auto_notiz(&splitflaechen.0, &flst_id);
            let status = AenderungenIntersection::get_auto_status(&splitflaechen.0, gebaeude_flst, &flst_id);
            if filter_only_modified && !status.was_modified() {
                return None;
            }
            let nutzung = ds_0.nutzung.clone();

            let mut eigentuemer = ds
                .iter()
                .map(|s| EigentuemerClean::from_str(&s.eigentuemer))
                .collect::<Vec<_>>();
            eigentuemer.sort();
            eigentuemer.dedup();

            Some((
                f,
                FlstEigentuemer {
                    nutzung,
                    status,
                    notiz: ds_0.notiz.clone(),
                    auto_notiz: notiz,
                    eigentuemer,
                },
            ))
        })
        .collect()
}

pub fn get_modified_fluren_flst(
    eigentuemer_map: &BTreeMap<FlstIdParsedNumber, FlstEigentuemer>,
    main_gemarkung: usize,
) -> BTreeMap<usize, Vec<FlstIdParsedNumber>> {
    let mut target_map = BTreeMap::new();
    for (e, v) in eigentuemer_map.iter() {
        if v.status.was_modified() && e.gemarkung == main_gemarkung {
            target_map
                .entry(e.flur)
                .or_insert_with(|| Vec::new())
                .push(e.clone());
        }
    }
    target_map
}

pub fn get_eigentuemer(
    eigentuemer_map: &BTreeMap<FlstIdParsedNumber, FlstEigentuemer>,
) -> Vec<(EigentuemerClean, Vec<String>)> {
    let mut target = BTreeMap::new();

    for (k, v) in eigentuemer_map.iter() {
        for e in v.eigentuemer.iter() {
            target
                .entry(e.clone())
                .or_insert_with(|| BTreeMap::new())
                .entry(k.flur)
                .or_insert_with(|| Vec::new())
                .push(k.clone());
        }
    }

    let mut target2 = Vec::new();
    for (eigentuemer, fluren) in target {
        target2.push((
            eigentuemer.clone(),
            fluren
                .iter()
                .filter_map(|(fl, flst)| Some(format!("Fl. {fl}: {}", join_flst(flst)?)))
                .collect::<Vec<_>>(),
        ));
    }
    target2
}

pub fn join_modified_fluren(
    modified: &BTreeMap<usize, Vec<FlstIdParsedNumber>>,
) -> BTreeMap<String, String> {
    modified
        .iter()
        .filter_map(|(k, v)| Some((format!("Fl. {k}: "), join_flst(v)?)))
        .collect()
}

pub fn generate_grafbat_out(
    info: &ProjektInfo,
    default_extent: &RissExtentReprojected,
    map: BTreeMap<usize, GrafbatOutConfig>,
) -> String {
    
    let mut mid = 1_usize;
    let mut pid = 1_usize;
    let mut lid = 1_usize;
    let mut txid = 1_usize;

    let zone = 33;

    let bbox = format!(
        "{min_x}.000000,{min_y}.000000,{max_x}.000000,{max_y}.000000", 
        min_x = update_dxf_x(zone, default_extent.min_x.floor()),
        max_x = update_dxf_x(zone, default_extent.max_x.ceil()),
        min_y = default_extent.min_y.floor(),
        max_y = default_extent.max_y.ceil(),
    );

    let mut header = format!(
        include_str!("./grafbat_header.txt"),
        prjname = info.antragsnr,
        uuid = uuid(),
        bbox = bbox,
    )
    .lines()
    .map(|s| s.to_string())
    .collect::<Vec<_>>();

    for (riss_id, outconf) in map.iter() {

        mid += 1;
        let menge_id_text_alt = mid.to_string();
        mid += 1;
        let menge_id_text_neu = mid.to_string();
        mid += 1;
        let menge_id_text_bleibt = mid.to_string();
        mid += 1;
        let menge_id_text_flst = mid.to_string();
        mid += 1;
        let menge_id_text_flur = mid.to_string();
        mid += 1;
        let menge_id_linien_rot = mid.to_string();
        mid += 1;
        let menge_id_punkte_untergehend = mid.to_string();
        mid += 1;
        let menge_id_gesamt = mid.to_string();

        let mut riss_items = Vec::new();

        // export texte
        let mut txtid_textalt = BTreeSet::new();
        for alt in outconf.aenderungen_texte_alt.iter() {
            txid += 1;
            header.push(format!(
                "TE{txid}: ,1600.9101.4140,{xcoord},{ycoord},{xcoord2},{ycoord2},{gon},0,0,0,0,,0,,,,,,,j,,,", 
                xcoord = update_dxf_x(zone, alt.optimized.pos.x),
                ycoord = alt.optimized.pos.y,
                xcoord2 = if alt.needs_bezug() { update_dxf_x(zone, alt.optimized.ref_pos.x).to_string() } else { String::new() },
                ycoord2 = if alt.needs_bezug() { alt.optimized.ref_pos.y.to_string() } else { String::new() },
                gon = 100.0,
            ));
            header.push(format!("  TX{txid}: {}", alt.optimized.kuerzel));
            txtid_textalt.insert(txid);
            riss_items.push(format!("TE={txid}"));
        }

        let mut txtid_textneu = BTreeSet::new();
        for neu in outconf.aenderungen_texte_neu.iter() {
            txid += 1;
            header.push(format!(
                "TE{txid}: ,1600.9101.4140,{xcoord},{ycoord},{xcoord2},{ycoord2},{gon},0,0,0,0,,0,,,,,,,n,,,0000ff", 
                xcoord = update_dxf_x(zone, neu.optimized.pos.x),
                ycoord = neu.optimized.pos.y,
                xcoord2 = if neu.needs_bezug() { update_dxf_x(zone, neu.optimized.ref_pos.x).to_string() } else { String::new() },
                ycoord2 = if neu.needs_bezug() { neu.optimized.ref_pos.y.to_string() } else { String::new() },
                gon = 100.0,
            ));
            header.push(format!("  TX{txid}: {}", neu.optimized.kuerzel));
            txtid_textneu.insert(txid);
            riss_items.push(format!("TE={txid}"));
        }

        let mut txtid_textbleibt = BTreeSet::new();
        for bleibt in outconf.aenderungen_texte_bleibt.iter() {
            txid += 1;
            header.push(format!(
                "TE{txid}: ,1600.9101.4140,{xcoord},{ycoord},{xcoord2},{ycoord2},{gon},0,0,0,0,,0,,,,,,,n,,,010101", 
                xcoord = update_dxf_x(zone, bleibt.optimized.pos.x),
                ycoord = bleibt.optimized.pos.y,
                xcoord2 = if bleibt.needs_bezug() { update_dxf_x(zone, bleibt.optimized.ref_pos.x).to_string() } else { String::new() },
                ycoord2 = if bleibt.needs_bezug() { bleibt.optimized.ref_pos.y.to_string() } else { String::new() },
                gon = 100.0,
            ));
            header.push(format!("  TX{txid}: {}", bleibt.optimized.kuerzel));
            txtid_textbleibt.insert(txid);
            riss_items.push(format!("TE={txid}"));
        }

        let mut txtid_flurstuecke = BTreeSet::new();
        for flst in outconf.flurstueck_texte.iter() {
            txid += 1;
            header.push(format!(
                "TE{txid}, ,0: ,1600.9102.4111,{xcoord},{ycoord},{xcoord2},{ycoord2},{gon},0,0,4,0,,0,,,,,,,n,,,", 
                xcoord = update_dxf_x(zone, flst.pos.x),
                ycoord = flst.pos.y,
                xcoord2 = if flst.needs_bezug() { update_dxf_x(zone, flst.ref_pos.x).to_string() } else { String::new() },
                ycoord2 = if flst.needs_bezug() { flst.ref_pos.y.to_string() } else { String::new() },
                gon = 100.0,
            ));
            header.push(format!("  TX{txid}: {}", flst.kuerzel));
            txtid_flurstuecke.insert(txid);
            riss_items.push(format!("TE={txid}"));
        }

        let mut txtid_flur = BTreeSet::new();
        for fl in outconf.flur_texte.iter() {
            txid += 1;
            header.push(format!(
                "TE{txid}, ,0: ,1600.9103.4200,{xcoord},{ycoord},,,{gon},0,0,4,0,,0,,,,,,,n,,,", 
                xcoord = update_dxf_x(zone, fl.pos.x),
                ycoord = fl.pos.y,
                gon = 100.0,
            ));
            header.push(format!("  TX{txid}: {}", fl.kuerzel));
            txtid_flur.insert(txid);
            riss_items.push(format!("TE={txid}"));
        }

        let mut txtid_linien_rot = BTreeSet::new();
        for rote_linie in outconf.aenderungen_rote_linien.iter() {
            for win in rote_linie.points.windows(2) {
                match &win {
                    &[pid_start, pid_end] => {

                        pid += 1;
                        let pid_start_save = pid;
                        header.push(format!("PK{pid}: ,1600.9104.0,{x},{y},,,0,0,,,,1005,09.10.24,0,,0,,0,0,,1,0,0,0,,,,,,", x = update_dxf_x(zone, pid_start.x), y = pid_start.y));
                        riss_items.push(format!("PK={pid}"));
                        txtid_linien_rot.insert(format!("PK={pid}"));

                        pid += 1;
                        let pid_end_save = pid;
                        header.push(format!("PK{pid}: ,1600.9104.0,{x},{y},,,0,0,,,,1005,09.10.24,0,,0,,0,0,,1,0,0,0,,,,,,", x = update_dxf_x(zone, pid_end.x), y = pid_end.y));
                        riss_items.push(format!("PK={pid}"));
                        txtid_linien_rot.insert(format!("PK={pid}"));

                        lid += 1;
                        header.push(format!("LI{lid}: PK={pid_start_save},PK={pid_end_save},1600.9104.1,,,,0,0,,,,"));
                        riss_items.push(format!("LI={lid}"));
                        txtid_linien_rot.insert(format!("LI={lid}"));
                    },
                    _ => { },
                }
            }
        }

        let mut punkte_id_untergehend = BTreeSet::new();
        for (p, angle) in lines_to_points(&outconf.aenderungen_nutzungsarten_linien) {
            pid += 1;
            let ang = Into::<angular_units::Gon<f64>>::into(angular_units::Deg(angle - 45.0));
            header.push(format!("PK{pid}: ,1600.401.20,{x},{y},,{gon},0,0,,,,1007,09.10.24,0,,0,,0,0,,1,0,0,0,,,,,,", x = update_dxf_x(zone, p.x), y = p.y, gon = ang.0));
            riss_items.push(format!("PK={pid}"));
            punkte_id_untergehend.insert(format!("PK={pid}"));
        }
        
        riss_items.sort();
        riss_items.dedup();
        
        header.push(format!("MA{menge_id_gesamt}: RISS{riss_id:03}-GESAMT,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in riss_items.iter() {
            header.push(format!("  MR: {i}")); 
        }

        // Mengen
        header.push(format!("MA{menge_id_text_alt}: Riss{riss_id}-Texte-Alt,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in txtid_textalt.iter() {
            header.push(format!("  MR: TE={i}"));
        }

        header.push(format!("MA{menge_id_text_neu}: Riss{riss_id}-Texte-Neu,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in txtid_textneu.iter() {
            header.push(format!("  MR: TE={i}"));
        }

        header.push(format!("MA{menge_id_text_bleibt}: Riss{riss_id}-Texte-Bleibt,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in txtid_textbleibt.iter() {
            header.push(format!("  MR: TE={i}")); 
        }

        header.push(format!("MA{menge_id_text_flst}: Riss{riss_id}-Texte-Flurstuecke,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in txtid_flurstuecke.iter() {
            header.push(format!("  MR: TE={i}")); 
        }

        header.push(format!("MA{menge_id_text_flur}: Riss{riss_id}-Texte-Flur,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in txtid_flur.iter() {
            header.push(format!("  MR: TE={i}")); 
        }

        header.push(format!("MA{menge_id_linien_rot}: Riss{riss_id}-Linien-Rot,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in txtid_linien_rot.iter() {
            header.push(format!("  MR: {i}")); 
        }

        header.push(format!("MA{menge_id_punkte_untergehend}: Riss{riss_id}-Punkte-Untergehend,,\"\",date:08.10.24,depend:1,neu:1"));
        for i in punkte_id_untergehend.iter() {
            header.push(format!("  MR: {i}")); 
        }

        // Plotbox
        header.push(
            format!(
                "PB{riss_id}: Riss{riss_id},1600.873.0,{min_x},{min_y},{max_x},{min_y},{hoehe},0", 
                min_x = update_dxf_x(zone, outconf.extent.min_x), 
                max_x = update_dxf_x(zone, outconf.extent.max_x),
                min_y = outconf.extent.min_y, 
                hoehe = outconf.extent.height_m()
            )
        );

    }

    header.join("\r\n")
}

fn join_flst(v: &Vec<FlstIdParsedNumber>) -> Option<String> {
    let mut v = v.clone();
    v.sort_by(|a, b| a.get_comma_f32().total_cmp(&b.get_comma_f32()));

    let (mut i, first) = match v.get(0) {
        Some(s) => (s.get_comma_f32(), s.clone()),
        None => return None,
    };

    if v.len() == 1 {
        return Some(first.format_str());
    }

    let mut target = vec![first.format_str()];
    let mut last = first;
    for q in v.iter().skip(1).take(v.len() - 1) {
        if (i.floor() + 1.0) as usize != (q.get_comma_f32().floor()) as usize {
            target.push(last.format_str());
            target.push(q.format_str());
        }
        last = q.clone();
        i = q.get_comma_f32();
    }

    if let Some(l) = v.last() {
        target.push(l.format_str());
    }

    Some(
        target
            .chunks(2)
            .filter_map(|w| match &w {
                &[a, b] => {
                    if a == b {
                        Some(a.trim().to_string())
                    } else {
                        Some(format!("{a} - {b}"))
                    }
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn join_fluren(v: &[usize]) -> Option<String> {
    let mut v = v.to_vec();
    v.sort();
    v.dedup();

    let (mut i, first) = match v.get(0) {
        Some(s) => (*s, *s),
        None => return None,
    };

    if v.len() == 1 {
        return Some(first.to_string());
    }

    let mut target = vec![first.to_string()];
    let mut last = first;
    for q in v.iter().skip(1).take(v.len() - 1) {
        if i + 1 != *q {
            target.push(last.to_string());
            target.push(q.to_string());
        }
        last = *q;
        i = *q;
    }

    if let Some(l) = v.last() {
        target.push(l.to_string());
    }

    Some(
        target
            .chunks(2)
            .filter_map(|w| match &w {
                &[a, b] => {
                    if a == b {
                        Some(a.trim().to_string())
                    } else {
                        Some(format!("{a} - {b}"))
                    }
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

pub fn calc_splitflaechen(
    aenderungen: &Aenderungen,
    split_nas: &SplitNasXml,
    _original_xml: &NasXMLFile,
    csv: &CsvDataType,
) -> AenderungenIntersections {
    let qt = split_nas.create_quadtree();

    let aenderungen = AenderungenClean {
        nas_xml_quadtree: qt,
        aenderungen: aenderungen.clone(),
    };

    log_status(&format!("Verschneide Änderungen.."));

    aenderungen.get_aenderungen_intersections(crate::get_main_gemarkung(csv))
}

pub const PADDING: f32 = 16.5 * 2.0;
pub const SCALE: f64 = 3500.0;

pub fn get_default_riss_extent(
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &[TaggedPolygon],
    crs: &str,
) -> Option<RissConfig> {
    let s_first = splitflaechen.first().map(|s| s.poly_cut.get_rect());
    let mut default_riss_extent_rect = match s_first {
        Some(s) => s,
        None => gebaeude.first().map(|g| g.poly.get_rect())?,
    };
    for sf in splitflaechen.iter().skip(1) {
        default_riss_extent_rect = default_riss_extent_rect.union(&sf.poly_cut.get_rect());
    }
    for p in gebaeude.iter() {
        default_riss_extent_rect = default_riss_extent_rect.union(&p.poly.get_rect());
    }
    let utm_center = default_riss_extent_rect.get_center();
    let latlon_center = crate::pdf::reproject_point_back_into_latlon(
        &SvgPoint {
            x: utm_center.x,
            y: utm_center.y,
        },
        &crs,
    )
    .unwrap_or_default();
    Some(RissConfig {
        lat: latlon_center.y,
        lon: latlon_center.x,
        crs: "latlon".to_string(),
        width_mm: (default_riss_extent_rect.get_width() / SCALE * 1000.0).round() as f32
            + PADDING
            + 10.0,
        height_mm: (default_riss_extent_rect.get_height() / SCALE * 1000.0).round() as f32
            + PADDING
            + 10.0,
        scale: SCALE as f32,
        rissgebiet: None,
    })
}

fn get_default_riss_extent_2(
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &[TaggedPolygon],
    split_nas: &SplitNasXml,
) -> Result<RissExtentReprojected, ()> {

    let default_riss_config = match get_default_riss_extent(splitflaechen, &gebaeude, &split_nas.crs) {
        Some(s) => s,
        None => return Err(()),
    };

    let riss = default_riss_config;

    let riss_extent = match riss.get_extent(&split_nas.crs, 0.0) {
        Some(s) => s,
        None => return Err(()),
    };

    let riss_extent_reprojected = match riss_extent.reproject(&split_nas.crs) {
        Some(s) => s,
        None => return Err(()),
    };

    Ok(riss_extent_reprojected)
}

pub fn export_splitflaechen(
    files: &mut Vec<(Option<String>, PathBuf, Vec<u8>)>,
    info: &ProjektInfo,
    csv: &CsvDataType,
    konfiguration: &Konfiguration,
    parent_dir: Option<String>,
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &[TaggedPolygon],
    split_nas: &SplitNasXml,
    nas_xml: &NasXMLFile,
    riss: Option<RissConfig>,
    num_riss: usize,
    total_risse: usize,
    lq_flurstuecke: &LinienQuadTree,
    lq_flurstuecke_und_nutzungsarten: &LinienQuadTree,
    hintergrund_cache: &mut HintergrundCache,
) -> Result<(usize, GrafbatOutConfig), ()> {
    let pdir_name = parent_dir.as_deref().unwrap_or("Aenderungen");

    let default_riss_config = match get_default_riss_extent(splitflaechen, &gebaeude, &split_nas.crs) {
        Some(s) => s,
        None => return Err(()),
    };

    let riss = riss.clone().unwrap_or(default_riss_config);

    let riss_extent = match riss.get_extent(&split_nas.crs, 0.0) {
        Some(s) => s,
        None => return Err(()),
    };

    let riss_extent_with_border = match riss.get_extent(&split_nas.crs, PADDING.into()) {
        Some(s) => s,
        None => return Err(()),
    };

    let riss_extent_reprojected = match riss_extent.reproject(&split_nas.crs) {
        Some(s) => s,
        None => return Err(()),
    };

    let riss_extent_with_border_reprojected =
        match riss_extent_with_border.reproject(&split_nas.crs) {
            Some(s) => s,
            None => return Err(()),
        };

    let riss_extent_cutpoly_noborder = riss_extent_reprojected.get_poly();

    let riss_extent_cutpoly_withborder = riss_extent_with_border_reprojected.get_poly();

    let calc_pdf_preview = HeaderCalcConfig::from_csv(
        &split_nas,
        csv,
        &Some(riss_extent_cutpoly_withborder.clone()),
    );

    let calc_pdf_final =
        HeaderCalcConfig::from_csv(&split_nas, csv, &Some(riss_extent_cutpoly_noborder.clone()));

    let riss_rect = riss_extent_reprojected.get_rect();
    let splitflaechen2 = splitflaechen
        .iter()
        .filter_map(|s| {
            if s.poly_cut.get_rect().overlaps_rect(&riss_rect) {
                if let Some(rg) = riss_extent_reprojected.rissgebiet.as_ref() {
                    if s.poly_cut.overlaps(&rg) || rg.overlaps(&s.poly_cut) {
                        Some(s)
                    } else {
                        None
                    }
                } else {
                    Some(s)
                }
            } else {
                None
            }
        })
        .cloned()
        .collect::<Vec<_>>();

    let gebaeude = gebaeude
    .iter()
    .filter_map(|s| {
        if s.poly.get_rect().overlaps_rect(&riss_rect) {
            if let Some(rg) = riss_extent_reprojected.rissgebiet.as_ref() {
                if s.poly.overlaps(&rg) || rg.overlaps(&s.poly) {
                    Some(s)
                } else {
                    None
                }
            } else {
                Some(s)
            }
        } else {
            None
        }
    })
    .cloned()
    .collect::<Vec<_>>();

    // TODO: accurate?
    let alle_flurstuecke = splitflaechen2
        .iter()
        .map(|s| s.flst_id.clone())
        .collect::<BTreeSet<_>>();
    let splitflaechen = splitflaechen
        .iter()
        .filter_map(|s| {
            if alle_flurstuecke.contains(&s.flst_id) {
                Some(s)
            } else {
                None
            }
        })
        .cloned()
        .collect::<Vec<_>>();

    log_status(&format!(
        "[{num_riss} / {total_risse}] Export {} Teilflächen",
        splitflaechen.len()
    ));

    let header = generate_header_pdf(info, &calc_pdf_final, split_nas, num_riss, total_risse);
    files.push((Some("Risselemente".to_string()), format!("Blattkopf_{pdir_name}.pdf").into(), header));

    let legende = generate_legende_xlsx(&splitflaechen);
    files.push((Some("Risselemente".to_string()), format!("Legende_{pdir_name}.xlsx").into(), legende));

    let na_splitflaechen = get_na_splitflaechen(
        &splitflaechen,
        &split_nas,
        Some(riss_extent_reprojected.get_rect()),
    );
    let aenderungen_nutzungsarten_linien =
        get_aenderungen_nutzungsarten_linien(
            &gebaeude,
            &na_splitflaechen, 
            lq_flurstuecke
        );
    if !aenderungen_nutzungsarten_linien.is_empty() {
        files.push((
            Some("Punkte".to_string()),
            format!("Punkte_NAGrenze_Untergehend_{pdir_name}.dxf").into(),
            lines_to_points_dxf(&aenderungen_nutzungsarten_linien),
        ));
    }
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Linien für untergehende NA-Grenzen generiert.",
        aenderungen_nutzungsarten_linien.len()
    ));

    let aenderungen_nutzungsarten_linien_2 = aenderungen_nutzungsarten_linien.clone();

    let aenderungen_rote_linien =
        get_aenderungen_rote_linien(&splitflaechen, lq_flurstuecke_und_nutzungsarten);
    if !aenderungen_rote_linien.is_empty() {
        files.push((
            Some("Linien".to_string()),
            format!("Linien_Rot_{pdir_name}.dxf").into(),
            lines_to_dxf(&aenderungen_rote_linien),
        ));
    }
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} rote Linien generiert.",
        aenderungen_rote_linien.len()
    ));

    let aenderungen_rote_linien_2 = aenderungen_rote_linien.clone();

    let aenderungen_texte: Vec<TextPlacement> =
        AenderungenIntersections::get_texte(&splitflaechen, &riss_extent_cutpoly_noborder);
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Texte generiert",
        aenderungen_texte.len()
    ));

    let mini_split_nas = get_mini_nas_xml(split_nas, &riss_extent_reprojected);
    let flst = get_flurstuecke(nas_xml, &riss_extent_reprojected);
    let fluren = get_fluren(nas_xml, &Some(riss_extent_reprojected.get_rect()));
    let gebaeude = get_gebaeude(nas_xml, &riss_extent_reprojected);
    let riss_von = (num_riss, total_risse);

    let flur_texte = fluren
        .get_labels(&Some(riss_extent_reprojected.get_poly()))
        .into_iter()
        .map(|fl| TextPlacement {
            kuerzel: fl.text(&calc_pdf_final),
            status: TextStatus::StaysAsIs,
            area: 1000,
            pos: fl.pos,
            ref_pos: fl.pos,
            poly: SvgPolygonInner::default(),
        })
        .collect::<Vec<_>>();
    if !flur_texte.is_empty() {
        files.push((
            Some("Texte".to_string()),
            format!("Flur_Texte_{pdir_name}.dxf").into(),
            texte_zu_dxf_datei(&flur_texte),
        ));
    }
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Flur-Texte",
        flur_texte.len()
    ));

    let flurstueck_texte = flst.get_labels(&Some(riss_extent_reprojected.get_poly()));
    if !flurstueck_texte.is_empty() {
        files.push((
            Some("Texte".to_string()),
            format!("Flurstueck_Texte_{pdir_name}.dxf").into(),
            texte_zu_dxf_datei(&flurstueck_texte),
        ));
    }
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Flurstueck-Texte",
        flurstueck_texte.len()
    ));

    log_status(&format!(
        "[{num_riss} / {total_risse}] Optimiere Beschriftungen... {:?}",
        riss_von
    ));
    let aenderungen_texte_optimized = crate::optimize::optimize_labels(
        &mini_split_nas,
        &splitflaechen,
        &gebaeude,
        &[],
        &aenderungen_texte,
        &OptimizeConfig::new(&riss, &riss_extent_reprojected, 0.5 /* mm */),
    );

    let beschriftungen_optimized_linien = aenderungen_texte_optimized
        .iter()
        .filter_map(|s| s.get_line())
        .map(|(start, end)| SvgLine {
            points: vec![start, end],
        })
        .collect::<Vec<_>>();
    if !beschriftungen_optimized_linien.is_empty() {
        files.push((
            Some("Texte".to_string()),
            format!("Beschriftung_Linien_{pdir_name}.dxf").into(),
            lines_to_dxf(&beschriftungen_optimized_linien),
        ));
    }
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Beschriftungs-Linien generiert.",
        beschriftungen_optimized_linien.len()
    ));

    let aenderungen_texte_bleibt = aenderungen_texte_optimized
        .iter()
        .map(|s| &s.optimized)
        .filter(|sf| sf.status == TextStatus::StaysAsIs)
        .cloned()
        .collect::<Vec<_>>();
    files.push((
        Some("Texte".to_string()),
        format!("Texte_Bleibt_{pdir_name}.dxf").into(),
        texte_zu_dxf_datei(&aenderungen_texte_bleibt),
    ));
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Texte: bleibende Kürzel",
        aenderungen_texte_bleibt.len()
    ));

    let aenderungen_texte_alt = aenderungen_texte_optimized
        .iter()
        .map(|s| &s.optimized)
        .filter(|sf| sf.status == TextStatus::Old)
        .cloned()
        .collect::<Vec<_>>();
    files.push((
        Some("Texte".to_string()),
        format!("Texte_Alt_{pdir_name}.dxf").into(),
        texte_zu_dxf_datei(&aenderungen_texte_alt),
    ));
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Texte: alte Kürzel",
        aenderungen_texte_alt.len()
    ));

    let aenderungen_texte_neu = aenderungen_texte_optimized
        .iter()
        .map(|s| &s.optimized)
        .filter(|sf| sf.status == TextStatus::New)
        .cloned()
        .collect::<Vec<_>>();
    files.push((
        Some("Texte".to_string()),
        format!("Texte_Neu_{pdir_name}.dxf").into(),
        texte_zu_dxf_datei(&aenderungen_texte_neu),
    ));
    log_status(&format!(
        "[{num_riss} / {total_risse}] {} Texte: neue Kürzel",
        aenderungen_texte_neu.len()
    ));

    log_status(&format!(
        "[{num_riss} / {total_risse}] Generiere PDF-Vorschau..."
    ));
    let pdf_vorschau = crate::pdf::generate_pdf_internal(
        Vec::new(),
        riss_von,
        info,
        &calc_pdf_preview,
        konfiguration,
        split_nas,
        &riss,
        &riss_extent_reprojected,
        // TODO: riss_extent_reprojected_noborder
        &aenderungen_rote_linien,
        &aenderungen_nutzungsarten_linien,
        &aenderungen_texte_optimized,
        &fluren,
        &flst,
        &gebaeude,
    );

    files.push((
        Some("Vorschau".to_string()),
        format!(
            "Vorschau_{}.pdf",
            parent_dir.as_deref().unwrap_or("Aenderungen")
        )
        .into(),
        pdf_vorschau,
    ));
    log_status(&format!(
        "[{num_riss} / {total_risse}] OK: PDF Vorschau generiert."
    ));

    let splitflaechen = AenderungenIntersections(splitflaechen.to_vec()).get_future_flaechen();
    let split_nas = split_nas.migrate_future(&splitflaechen.0);
    let aenderungen_texte =
        AenderungenIntersections::get_texte(&splitflaechen.0, &riss_extent_cutpoly_noborder);
    let mini_split_nas = get_mini_nas_xml(&split_nas, &riss_extent_reprojected);
    let aenderungen_texte_optimized_new = crate::optimize::optimize_labels(
        &mini_split_nas,
        &splitflaechen.0,
        &gebaeude,
        &[],
        &aenderungen_texte,
        &OptimizeConfig::new(&riss, &riss_extent_reprojected, 0.5 /* mm */),
    );

    log_status(&format!(
        "[{num_riss} / {total_risse}] Generiere Hintergrund-Vorschau..."
    ));
    log_status(&format!(
        "[{num_riss} / {total_risse}] id = {:?}",
        riss.get_id()
    ));
    let hintergrund_vorschau = crate::pdf::generate_pdf_internal(
        hintergrund_cache
            .images
            .remove(&riss.get_id())
            .unwrap_or_default(),
        (num_riss, total_risse),
        info,
        &calc_pdf_preview,
        konfiguration,
        &split_nas,
        &riss,
        &riss_extent_reprojected,
        // TODO: riss_extent_reprojected_noborder
        &Vec::new(),
        &Vec::new(),
        &aenderungen_texte_optimized_new,
        &fluren,
        &flst,
        &gebaeude,
    );

    files.push((
        Some("Vorschau".to_string()),
        format!(
            "Vorschau_mit_Hintergrund_{}.pdf",
            parent_dir.as_deref().unwrap_or("Aenderungen")
        )
        .into(),
        hintergrund_vorschau,
    ));
    log_status(&format!(
        "[{num_riss} / {total_risse}] OK: PDF Vorschau mit Hintergrund generiert."
    ));



    let aenderungen_texte_neu_2 = aenderungen_texte_optimized
        .iter()
        .filter(|s| s.optimized.status == TextStatus::New)
        .cloned()
        .collect::<Vec<_>>();

    let aenderungen_texte_alt_2 = aenderungen_texte_optimized
        .iter()
        .filter(|s| s.optimized.status == TextStatus::Old)
        .cloned()
        .collect::<Vec<_>>();

    let aenderungen_texte_bleibt_2 = aenderungen_texte_optimized
        .iter()
        .filter(|s| s.optimized.status == TextStatus::StaysAsIs)
        .cloned()
        .collect::<Vec<_>>();

    Ok((num_riss, GrafbatOutConfig {
        extent: riss_extent_with_border_reprojected,
        aenderungen_rote_linien: aenderungen_rote_linien_2.clone(),
        aenderungen_nutzungsarten_linien: aenderungen_nutzungsarten_linien_2.clone(),
        aenderungen_texte_neu: aenderungen_texte_neu_2.clone(),
        aenderungen_texte_alt: aenderungen_texte_alt_2.clone(),
        aenderungen_texte_bleibt: aenderungen_texte_bleibt_2.clone(),
        flurstueck_texte: flurstueck_texte.clone(),
        flur_texte: flur_texte.clone(),
    }))
}

pub struct GrafbatOutConfig {
    extent: RissExtentReprojected,
    aenderungen_rote_linien: Vec<SvgLine>,
    aenderungen_nutzungsarten_linien: Vec<SvgLine>,
    aenderungen_texte_neu: Vec<OptimizedTextPlacement>,
    aenderungen_texte_alt: Vec<OptimizedTextPlacement>,
    aenderungen_texte_bleibt: Vec<OptimizedTextPlacement>,
    flurstueck_texte: Vec<TextPlacement>,
    flur_texte: Vec<TextPlacement>,
}



pub struct LinienQuadTree {
    pub linien: Vec<(SvgPoint, SvgPoint)>,
    pub qt: quadtree_f32::QuadTree,
}

impl LinienQuadTree {
    pub fn new(linien: Vec<(SvgPoint, SvgPoint)>) -> Self {
        let items = linien
            .iter()
            .enumerate()
            .filter_map(|(id, (a, b))| {
                let a = a.round_to_3dec();
                let b = b.round_to_3dec();
                if a.equals(&b) {
                    return None;
                }
                Some((id, points_to_rect(&(a, b))))
            })
            .collect::<Vec<_>>();

        let max_items = items.len().saturating_div(20).max(500);

        let qt = quadtree_f32::QuadTree::new_with_max_items_per_quad(
            items
                .iter()
                .map(|(k, v)| (quadtree_f32::ItemId(*k), quadtree_f32::Item::Rect(*v))),
            max_items,
        );

        Self { linien, qt }
    }

    fn line_overlaps_or_equals(&self, a: &SvgPoint, b: &SvgPoint) -> bool {
        let rect = points_to_rect(&(*a, *b));
        let items = self
            .qt
            .get_ids_that_overlap(&rect)
            .into_iter()
            .filter_map(|i| self.linien.get(i.0))
            .cloned()
            .collect::<Vec<_>>();
        for (s, t) in items.iter() {
            let a_on_line =
                a.equals(s) || a.equals(t) || crate::ui::dist_to_segment(*a, *s, *t).distance < 0.1;
            let b_on_line =
                b.equals(s) || b.equals(t) || crate::ui::dist_to_segment(*b, *s, *t).distance < 0.1;
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
            _ => {}
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
        max_x,
        max_y,
        min_x,
        min_y,
    }
}

pub fn get_aenderungen_rote_linien(
    splitflaechen: &[AenderungenIntersection],
    linienquadtree: &LinienQuadTree,
) -> Vec<SvgLine> {
    // rote linie: neue linie, die nicht auf nas xml linie liegt (oder teil einer nas xml linie ist)
    //
    // -> create btree
    // -> select linien
    // -> check if overlaps linie
    // -> deduplicate + join ends

    let mut alle_linien_zu_checken = splitflaechen
        .iter()
        .flat_map(|s| {
            let poly_cut = nas::cleanup_poly(&s.poly_cut);
            poly_cut.iter().flat_map(|s| {
                let mut lines = l_to_points(&s.outer_ring);
                lines.extend(s.inner_rings.iter().flat_map(l_to_points));
                lines
            }).collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
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

    lines_end
        .iter()
        .map(|s| SvgLine {
            points: vec![s.0, s.1],
        })
        .collect::<Vec<_>>()

    // merge_lines_again(lines_end) // TODO: buggy!
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
                }
            };

            let last = match q.last().map(|s| s.1) {
                Some(s) => s,
                None => {
                    modified_mark_remove.insert(i);
                    continue;
                }
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
                    }
                };

                let k_last = match k.last().map(|s| s.1) {
                    Some(s) => s,
                    None => {
                        modified_mark_remove.insert(p);
                        continue;
                    }
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

    v.into_iter()
        .filter_map(|p| {
            let mut points = p
                .into_iter()
                .flat_map(|(a, b)| vec![a, b])
                .collect::<Vec<_>>();
            points.dedup_by(|a, b| a.equals(b));
            if points.is_empty() {
                None
            } else {
                Some(SvgLine { points })
            }
        })
        .collect::<Vec<_>>()
}

pub fn get_na_splitflaechen(
    splitflaechen: &[AenderungenIntersection],
    split_nas: &SplitNasXml,
    rect: Option<quadtree_f32::Rect>,
) -> Vec<AenderungenIntersection> {
    let mut finalized = splitflaechen.to_vec();
    let existing_flst = splitflaechen
        .iter()
        .map(|f| &f.flst_id)
        .collect::<BTreeSet<_>>();
    for (_k, v) in split_nas.flurstuecke_nutzungen.iter() {
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

            let ebene = q.get_ebene()?;
            let obj_id = q.attributes.get("id")?;
            let alt_kuerzel = q.get_auto_kuerzel()?;
            let intersect_id = q
                .attributes
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

pub fn get_aenderungen_nutzungsarten_linien(
    gebaeude: &[TaggedPolygon],
    splitflaechen: &[AenderungenIntersection],
    lq: &LinienQuadTree,
) -> Vec<SvgLine> {
    let mut pairs = BTreeSet::new();

    for (id1, s1) in splitflaechen.iter().enumerate() {
        let rect = s1.poly_cut.get_rect();
        let it = splitflaechen.iter().enumerate().filter_map(|(i, p)| {
            if p.poly_cut.get_rect().overlaps_rect(&rect) {
                Some((i, p))
            } else {
                None
            }
        });

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
        let apoly = nas::cleanup_poly(&a.poly_cut);
        let bpoly = nas::cleanup_poly(&b.poly_cut);

        for mut apoly in apoly {
            for mut bpoly in bpoly.iter().cloned() {
                apoly.correct_almost_touching_points(&bpoly, 0.1, true);
                apoly.insert_points_from(&bpoly, 0.1);
                bpoly.insert_points_from(&apoly, 0.1);
                let shared_lines = get_shared_lines(&apoly, &bpoly);
                let mut shared_lines_2 = shared_lines
                    .into_iter()
                    .filter_map(|s| {
                        let first = s.points.first()?;
                        let last = s.points.last()?;
                        if lq.line_overlaps_or_equals(first, last) {
                            None
                        } else {
                            Some(s)
                        }
                    })
                    .collect::<Vec<_>>();
        
                v.append(&mut shared_lines_2);
            }
        }
    }

    for geb in gebaeude.iter() {
        v.push(geb.poly.outer_ring.clone());
        for i in geb.poly.inner_rings.iter() {
            v.push(i.clone());  
        }
    }

    v
}

fn get_shared_lines(a: &SvgPolygonInner, b: &SvgPolygonInner) -> Vec<SvgLine> {
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

    map.into_iter()
        .map(|((ax, ay), (bx, by))| SvgLine {
            points: vec![
                SvgPoint {
                    x: (*ax as f64) / 1000.0,
                    y: (*ay as f64) / 1000.0,
                },
                SvgPoint {
                    x: (*bx as f64) / 1000.0,
                    y: (*by as f64) / 1000.0,
                },
            ],
        })
        .collect()
}

pub fn get_linecoords(p: &SvgPolygonInner) -> BTreeSet<((u64, u64), (u64, u64))> {
    let mut lines = crate::geograf::l_to_points(&&p.outer_ring);
    lines.extend(p.inner_rings.iter().flat_map(crate::geograf::l_to_points));
    lines
        .into_iter()
        .flat_map(|(a, b)| {
            vec![
                (
                    ((a.x * 1000.0) as u64, (a.y * 1000.0) as u64),
                    ((b.x * 1000.0) as u64, (b.y * 1000.0) as u64),
                ),
                (
                    ((b.x * 1000.0) as u64, (b.y * 1000.0) as u64),
                    ((a.x * 1000.0) as u64, (a.y * 1000.0) as u64),
                ),
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

    let (mut doc, page1, layer1) = PdfDocument::new(&text, Mm(w), Mm(h), &text);
    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        ..Default::default()
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
        (
            printpdf::Point {
                x: Mm(0.0).into(),
                y: Mm(0.0).into(),
            },
            false,
        ),
        (
            printpdf::Point {
                x: Mm(w).into(),
                y: Mm(0.0).into(),
            },
            false,
        ),
        (
            printpdf::Point {
                x: Mm(w).into(),
                y: Mm(h).into(),
            },
            false,
        ),
        (
            printpdf::Point {
                x: Mm(0.0).into(),
                y: Mm(h).into(),
            },
            false,
        ),
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
            Mm(w - (padding * 1.5)).into_pt(),
            Mm(padding * 1.5).into_pt(),
            90.0,
        ));
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
    _split_nas: &SplitNasXml,
    num_riss: usize,
    total_risse: usize,
) -> Vec<u8> {
    let (mut doc, page1, layer1) =
        PdfDocument::new("Risskopf", Mm(175.0), Mm(35.0), "Risskopf Ebene 1");

    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        ..Default::default()
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

    let _ = crate::pdf::write_header(
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
        let f = self
            .gemarkungen
            .get(&self.gemarkungs_nr)
            .unwrap_or(&default);
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
                cl.iter()
                    .map(|s| s.format_str())
                    .collect::<Vec<_>>()
                    .join(", ")
                    .trim()
                    .to_string()
            }
        }
    }

    fn get_fluren_string_internal(&self) -> Vec<String> {
        let default = BTreeMap::new();
        let f = self
            .gemarkungen
            .get(&self.gemarkungs_nr)
            .unwrap_or(&default);
        f.keys().map(|s| s.to_string()).collect::<Vec<_>>()
    }

    pub fn get_fluren_string(&self) -> String {
        let internal = self.get_fluren_string_internal();
        if internal.len() > 4 {
            "diverse".to_string()
        } else {
            internal.join(", ").trim().to_string()
        }
    }

    pub fn get_fluren_len(&self) -> usize {
        self.get_fluren_string_internal().len()
    }

    pub fn from_csv(
        split_nas: &SplitNasXml,
        csv: &CsvDataType,
        extent_poly: &Option<SvgPolygonInner>,
    ) -> Self {
        let target_gemarkung_nr = crate::get_main_gemarkung(csv);

        let flst_overlaps = match extent_poly.as_ref() {
            Some(p) => {
                let mut s = p.clone();
                s.correct_winding_order();

                let qt = split_nas.create_quadtree();

                qt.get_overlapping_flst(&s.get_rect())
                    .into_iter()
                    .filter_map(|tp| {
                        let mut poly = tp.1.poly.clone();
                        poly.correct_winding_order();
                        if poly.is_completely_inside_of(&s) {
                            Some(tp.1)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            }
            None => split_nas
                .flurstuecke_nutzungen
                .values()
                .flat_map(|flst| flst.iter().cloned())
                .collect::<Vec<_>>(),
        };

        let mut flst_overlaps = flst_overlaps
            .into_iter()
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
            gemarkungen
                .entry(f.gemarkung)
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

pub fn generate_legende_xlsx(splitflaechen: &[AenderungenIntersection]) -> Vec<u8> {
    let alle_kuerzel = splitflaechen
        .iter()
        .flat_map(|s| vec![s.alt.clone(), s.neu.clone()].into_iter())
        .collect::<BTreeSet<_>>();

    let map = crate::get_nutzungsartenkatalog();

    let mut lines = alle_kuerzel
        .iter()
        .filter_map(|k| {
            let bez = &map.get(k)?.bez;
            Some(format!("{bez} ({k})"))
        })
        .collect::<Vec<_>>();

    lines.sort();

    crate::xml_templates::generate_legende_xlsx(&crate::xml_templates::LegendeInfo {
        header: "Legende Abkürzungen".to_string(),
        zeilen: lines.clone(),
    })
}

pub fn append_shp(
    files: &mut Vec<(Option<String>, PathBuf, Vec<u8>)>,
    name: &str,
    parent_dir: Option<String>,
    shp_file: ShpReturn,
) {
    files.push((
        parent_dir.clone(),
        format!("{name}.shp").into(),
        shp_file.shp,
    ));
    files.push((
        parent_dir.clone(),
        format!("{name}.shx").into(),
        shp_file.shx,
    ));
    files.push((
        parent_dir.clone(),
        format!("{name}.dbf").into(),
        shp_file.dbf,
    ));
    files.push((
        parent_dir.clone(),
        format!("{name}.prj").into(),
        shp_file.prj,
    ));
    files.push((
        parent_dir.clone(),
        format!("{name}.cpg").into(),
        shp_file.cpg,
    ));
}
