use std::collections::{BTreeMap, BTreeSet};

use printpdf::path::PaintMode;
use printpdf::{CustomPdfConformance, Mm, PdfConformance, PdfDocument, PdfLayerReference, Rgb};
use serde_derive::{Deserialize, Serialize};
use web_sys::console::log_1;
use crate::geograf::get_aenderungen_rote_linien;
use crate::LatLng;
use crate::csv::CsvDataType;
use crate::nas::{
    parse_nas_xml, translate_from_geo_poly, translate_to_geo_poly, 
    NasXMLFile, SplitNasXml, SvgLine, SvgPoint, SvgPolygon, TaggedPolygon, UseRadians, LATLON_STRING
};
use crate::ui::{Aenderungen, AenderungenIntersection, PolyNeu};
use crate::xlsx::FlstIdParsed;
use crate::xml::{self, XmlNode};

pub type Risse = BTreeMap<String, RissConfig>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EbenenStyle {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fill_color: Option<String>,
    #[serde(default)]
    pub outline_color: Option<String>,
    #[serde(default)]
    pub outline_thickness: Option<f32>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Konfiguration {
    pub map: MapKonfiguration,
    #[serde(default)]
    pub style: StyleConfig,
    #[serde(default)]
    pub pdf: PdfStyleConfig,
    #[serde(default)]
    pub merge: MergeConfig,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    #[serde(default = "one")]
    pub stage1_maxdst_point: f64,
    #[serde(default = "one")]
    pub stage1_maxdst_line: f64,
    #[serde(default = "one")]
    pub stage2_maxdst_point: f64,
    #[serde(default = "one")]
    pub stage2_maxdst_line: f64,
    #[serde(default = "one")]
    pub stage3_maxdst_line: f64,
    #[serde(default = "zero_point_two")]
    pub stage3_maxdst_line2: f64,
    #[serde(default = "five")]
    pub stage3_maxdeviation_followline: f64,
}

fn one() -> f64 { 1.0 }
fn zero_point_two() -> f64 { 0.2 }
fn five() -> f64 { 5.0 }

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MapKonfiguration {
    #[serde(default)]
    pub basemap: Option<String>,
    #[serde(default)]
    pub dop_source: Option<String>,
    #[serde(default)]
    pub dop_layers: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    #[serde(default)]
    pub ebenen_ordnung: Vec<String>,
    #[serde(default)]
    pub ebenen: BTreeMap<String, EbenenStyle>,
}

impl StyleConfig {
    pub fn get_styles_sorted(&self) -> Vec<(String, EbenenStyle)> {
        self.ebenen_ordnung.iter().filter_map(|s| self.ebenen.get(s).cloned().map(|q| (s.clone(), q))).collect()
    }
}

pub type Kuerzel = String;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PdfStyleConfig {
    #[serde(default)]
    pub grenzpunkt_svg: Option<String>,
    #[serde(default)]
    pub pfeil_svg: Option<String>,
    #[serde(default)]
    pub nordpfeil_svg: Option<String>,
    #[serde(default)]
    pub gebauede_loeschen_svg: Option<String>,

    #[serde(default)]
    pub ax_flur_stil: PdfEbenenStyle,
    #[serde(default)]
    pub ax_bauraum_stil: PdfEbenenStyle,
    #[serde(default)]
    pub lagebez_mit_hsnr: PtoStil,

    #[serde(default)]
    pub layer_ordnung: Vec<String>,
    #[serde(default)]
    pub nutzungsarten: BTreeMap<String, PdfEbenenStyle>,
    #[serde(default)]
    pub beschriftungen: BTreeMap<String, PtoStil>,
    #[serde(default)]
    pub symbole: BTreeMap<String, PpoStil>,
}

impl PdfStyleConfig {
    pub fn get_nutzungsarten_sorted(&self) -> Vec<(String, PdfEbenenStyle)> {
        self.layer_ordnung.iter().filter_map(|s| self.nutzungsarten.get(s).cloned().map(|q| (s.clone(), q))).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfEbenenStyle {
    #[serde(default)]
    pub kuerzel: String,
    #[serde(default)]
    pub fill_color: Option<String>,
    #[serde(default = "default_fill")]
    pub fill: bool,
    #[serde(default)]
    pub outline_color: Option<String>,
    #[serde(default)]
    pub outline_thickness: Option<f32>,
    #[serde(default)]
    pub outline_dash: Option<String>,
    #[serde(default)]
    pub outline_overprint: bool,
    #[serde(default)]
    pub pattern_svg: Option<String>,
    #[serde(default)]
    pub pattern_placement: Option<String>,
    #[serde(default)]
    pub lagebez_ohne_hsnr: PtoStil,
}

fn default_fill() -> bool { true }

impl Default for PdfEbenenStyle {
    fn default() -> Self {
        PdfEbenenStyle {
            kuerzel: String::new(),
            fill_color: Some("#000000".to_string()),
            fill: true,
            outline_color: None,
            outline_thickness: None,
            outline_overprint: false,
            outline_dash: None,
            pattern_svg: None,
            pattern_placement: None,
            lagebez_ohne_hsnr: PtoStil::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PtoStil {
    #[serde(default)]
    pub art: String, // BEZ, Gewanne
    #[serde(default)]
    pub fontsize: Option<f32>, // 12
    #[serde(default)]
    pub font: Option<String>, // Arial
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PpoStil {
    #[serde(default)]
    pub art: String, // VEG, ...
    #[serde(default)]
    pub svgname: Option<String>, // wald.svg
    #[serde(default)]
    pub svg_base64: Option<String>, // ...
}

pub type RissMap = BTreeMap<String, RissExtent>;
pub type RissMapReprojected = BTreeMap<String, RissExtentReprojected>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RissExtent {
    pub coords: Vec<LatLng>,
    pub projection: String,
}

impl RissExtent {
    // latlon -> 
    pub fn reproject(&self, target_crs: &str, log: &mut Vec<String>) -> Option<RissExtentReprojected> {
        
        let mut coords = self.coords.iter().map(|l| {
            (l.lng.to_radians(), l.lat.to_radians(), 0.0)
        }).collect::<Vec<_>>();
        if coords.is_empty() {
            return None;
        }

        let source = proj4rs::Proj::from_proj_string(LATLON_STRING).ok()?;
        let target = proj4rs::Proj::from_proj_string(&target_crs).ok()?;
        proj4rs::transform::transform(&source, &target, coords.as_mut_slice()).ok()?;
        let points = coords.iter().map(|p| {
            SvgPoint {
                x: p.0, 
                y: p.1,
            }
        }).collect::<Vec<_>>();

        let mut max_x = points.get(0)?.x;
        let mut min_x = points.get(0)?.x;
        let mut max_y = points.get(0)?.y;
        let mut min_y = points.get(0)?.y;

        for p in points {
            if p.x > max_x { max_x = p.x; }
            if p.x < min_x { min_x = p.x; }
            if p.y > max_y { max_y = p.y; }
            if p.y < min_y { min_y = p.y; }
        }

        Some(RissExtentReprojected {
            crs: target_crs.to_string(),
            max_x,
            min_x,
            max_y,
            min_y,
        })

    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RissExtentReprojected {
    pub crs: String,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl RissExtentReprojected {
    pub fn width_m(&self) -> f64 {
        (self.max_x - self.min_x).abs()
    }
    pub fn height_m(&self) -> f64 {
        (self.max_y - self.min_y).abs()
    }
    pub fn get_rect(&self) -> quadtree_f32::Rect {
        quadtree_f32::Rect {
            min_x: self.min_x,
            min_y: self.min_y,
            max_x: self.max_x,
            max_y: self.max_y,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ProjektInfo {
    pub antragsnr: String,
    pub katasteramt: String,
    pub vermessungsstelle: String,
    pub erstellt_durch: String,
    pub beruf_kuerzel: String,
    pub gemeinde: String,
    pub gemarkung: String,
    pub gemarkung_nr: String,
    #[serde(default)]
    pub bearbeitung_beendet_am: String,
    #[serde(default)]
    pub alkis_aktualitaet: String,
    #[serde(default)]
    pub orthofoto_datum: String,
    #[serde(default)]
    pub gis_feldbloecke_datum: String,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RissConfig {
    pub lat: f64,
    pub lon: f64,
    pub crs: String,
    pub width_mm: f32,
    pub height_mm: f32,
    pub scale: f32,
}

pub enum Aenderung {
    GebauedeLoeschen {
        id: String,
    },
    NutzungAendern {
        nutzung_alt: String,
        nutzung_neu: String,
    },
    NutzungZerlegen {
        nutzung_alt: String,
        nutzung_neu: BTreeMap<SvgLine, String>,
    },
    RingAnpassen {
        neue_ringe: BTreeMap<String, SvgLine>,
    },
    RingLoeschen {
        ring_geloeschet: String,
    }
}

// + Risse config
// + Änderungen
pub fn generate_pdf(
    projekt_info: &ProjektInfo,
    konfiguration: &Konfiguration,
    csv: &CsvDataType, 
    xml_original: Vec<XmlNode>,
    aenderungen: &Aenderungen, 
    risse: &Risse,
    riss_map: &RissMap,
    log: &mut Vec<String>
) -> Vec<u8> {

    let len = risse.len();
    if len == 0 {
        return Vec::new();
    }

    let whitelist = crate::xml::get_all_nodes_in_tree(&xml_original)
        .iter()
        .filter(|n| n.node_type.starts_with("AX_"))
        .map(|n| n.node_type.clone())
        .collect::<Vec<_>>();

    let xml = match parse_nas_xml(xml_original, &whitelist, log) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let nas_cut_original = match crate::nas::split_xml_flurstuecke_inner(&xml, log) {
        Ok(o) => o,
        Err(e) => return Vec::new(),
    };

    // AX_Flurstueck, AX_Flur, AX_BauRaumBodenOrdnungsRecht

    generate_pdf_internal(
        projekt_info,
        konfiguration,
        &xml,
        &nas_cut_original,
        &[],
        risse,
        &riss_map.iter().filter_map(|(k, v)| Some((k.clone(), v.reproject(&xml.crs, log)?))).collect(),
        log,
    )
}

pub fn generate_pdf_internal(
    projekt_info: &ProjektInfo,
    konfiguration: &Konfiguration,
    xml: &NasXMLFile,
    nas_cut_original: &SplitNasXml,
    splitflaechen: &[AenderungenIntersection],
    risse: &Risse,
    riss_map_reprojected: &RissMapReprojected,
    log: &mut Vec<String>
) -> Vec<u8> {

    let first_riss_id = risse.keys().next().and_then(|s| s.split("-").next()).unwrap_or("");

    let (mut doc, page1, layer1) = PdfDocument::new(
        "Riss",
        Mm(risse.iter().next().map(|(k, v)| v.width_mm).unwrap_or(210.0)),
        Mm(risse.iter().next().map(|(k, v)| v.height_mm).unwrap_or(297.0)),
        &format!("Riss 1 / {} ({first_riss_id})", risse.len()),
    );

    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        .. Default::default()
    }));

    for (i, (ri, rc))  in risse.iter().enumerate() {

        let riss_extent = match riss_map_reprojected.get(ri) {
            Some(s) => s,
            None => continue,
        };

        let p = ri.split("-").next().unwrap_or("");
        let (page, layer) = if i == 0 {
            (page1, layer1)
        } else {
            doc.add_page(Mm(rc.width_mm), Mm(rc.height_mm), &format!("Riss {} / {} ({p})", risse.len(), i + 1))
        };

        let page = doc.get_page(page);
        let mut layer = page.get_layer(layer);

        let nutzungsarten = reproject_splitnas_into_pdf_space(
            &nas_cut_original,
            &riss_extent,
            rc,
            log
        );

        let _ = write_nutzungsarten(&mut layer, &nutzungsarten, &konfiguration, log);

        let flst = get_flurstuecke_in_pdf_space(
            &xml,
            &riss_extent,
            rc,
            log
        );

        let _ = write_flurstuecke(&mut layer, &flst, &konfiguration, log);

        // let _ = write_grenzpunkte(&mut layer, &flst, &konfiguration, log);

        let fluren = get_fluren_in_pdf_space(
            &flst,
            &riss_extent,
            rc,
            log
        );
        
        let _ = write_fluren(&mut layer, &fluren, &konfiguration, log);

        let _ = write_border(&mut layer, 16.5, &rc);
        
        web_sys::console::log_1(&"6...".into());

        let rote_linien = get_aenderungen_rote_linien(splitflaechen, xml, nas_cut_original)
        .into_iter().map(|l| {
            line_into_pdf_space(&l, riss_extent, rc, &mut Vec::new())
        }).collect::<Vec<_>>();

        let _ = write_rote_linien(&mut layer, &rote_linien);
        
        /*
        let nas_xml_in_pdf_space = reproject_nasxml_into_pdf_space(
            &xml,
            &riss_extent,
            rc,
            log,
        );
        */
    }

    doc.save_to_bytes().unwrap_or_default()
}



pub fn reproject_aenderungen_into_target_space(
    aenderungen: &Aenderungen,
    target_proj: &str,
) -> Result<Aenderungen, String> {

    use crate::nas::LATLON_STRING;

    let target_proj = proj4rs::Proj::from_proj_string(&target_proj)
    .map_err(|e| format!("source_proj_string: {e}: {:?}", target_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
    .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    Ok(Aenderungen {
        gebaeude_loeschen: aenderungen.gebaeude_loeschen.clone(),
        na_definiert: aenderungen.na_definiert.clone(),
        na_polygone_neu: aenderungen.na_polygone_neu
        .iter()
        .map(|(k, v)| {
            (k.clone(), PolyNeu {
                poly: crate::nas::reproject_poly(&v.poly, &latlon_proj, &target_proj, UseRadians::ForSourceAndTarget),
                nutzung: v.nutzung.clone(),
            })
        })
        .collect()
    })
}

pub fn reproject_poly_back_into_latlon(
    poly: &SvgPolygon,
    source_proj: &str,
) -> Result<SvgPolygon, String> {

    let source_proj = proj4rs::Proj::from_proj_string(&source_proj)
    .map_err(|e| format!("source_proj_string: {e}: {:?}", source_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
    .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    Ok(crate::nas::reproject_poly(poly, &source_proj, &latlon_proj, UseRadians::None))
}

pub fn reproject_aenderungen_back_into_latlon(
    aenderungen: &Aenderungen,
    source_proj: &str,
) -> Result<Aenderungen, String> {

    use crate::nas::LATLON_STRING;

    let source_proj = proj4rs::Proj::from_proj_string(&source_proj)
    .map_err(|e| format!("source_proj_string: {e}: {:?}", source_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
    .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    Ok(Aenderungen {
        gebaeude_loeschen: aenderungen.gebaeude_loeschen.clone(),
        na_definiert: aenderungen.na_definiert.clone(),
        na_polygone_neu: aenderungen.na_polygone_neu
        .iter()
        .map(|(k, v)| {
            (k.clone(), PolyNeu {
                poly: crate::nas::reproject_poly(&v.poly, &source_proj, &latlon_proj, UseRadians::None),
                nutzung: v.nutzung.clone(),
            })
        })
        .collect()
    })
}


pub fn reproject_splitflaechen_into_pdf_space(
    splitflaechen: &[AenderungenIntersection],
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> Result<Vec<AenderungenIntersection>, String> {
    let target_riss = riss.get_rect();
    Ok(splitflaechen.iter().map(|s| AenderungenIntersection {
        alt: s.alt.clone(),
        neu: s.neu.clone(),
        flst_id: s.flst_id.clone(),
        poly_cut: poly_into_pdf_space(&s.poly_cut, &riss, riss_config, log),
    }).collect())
}

#[inline(always)]
fn reproject_splitnas_into_pdf_space(
    split_flurstuecke: &SplitNasXml,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> SplitNasXml {
    let target_riss = quadtree_f32::Rect {
        min_x: riss.min_x,
        min_y: riss.min_y,
        max_x: riss.max_x,
        max_y: riss.max_y,
    };
    SplitNasXml {
        crs: "pdf".to_string(),
        flurstuecke_nutzungen: split_flurstuecke.flurstuecke_nutzungen.iter().filter_map(|(k, v)| {
            let v = v.iter().filter_map(|s| {
                if s.get_rect().overlaps_rect(&target_riss) {
                    Some(TaggedPolygon {
                        attributes: s.attributes.clone(),
                        poly: poly_into_pdf_space(&s.poly, &riss, riss_config, log),
                    })
                } else {
                    None
                }
            }).collect::<Vec<_>>();
            if v.is_empty() {
                None 
            } else {
                Some((k.clone(), v))
            }
        }).collect()
    }
}

#[inline(always)]
fn reproject_nasxml_into_pdf_space(
    nas_xml: &NasXMLFile,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> NasXMLFile {
    NasXMLFile {
        crs: "pdf".to_string(),
        ebenen: nas_xml.ebenen.iter().map(|(k, v)| {
            (k.clone(),
            v.iter().map(|s| {
                TaggedPolygon {
                    attributes: s.attributes.clone(),
                    poly: poly_into_pdf_space(&s.poly, &riss, riss_config, log),
                }
            }).collect())
        }).collect()
    }
}

fn poly_into_pdf_space(
    poly: &SvgPolygon,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>,
) -> SvgPolygon {
    SvgPolygon { 
        outer_rings: poly.outer_rings.iter().map(|l| line_into_pdf_space(l, riss, riss_config, log)).collect(), 
        inner_rings: poly.inner_rings.iter().map(|l| line_into_pdf_space(l, riss, riss_config, log)).collect(), 
    }
}

fn line_into_pdf_space(
    line: &SvgLine,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>,
) -> SvgLine {
    SvgLine {
        points: line.points.iter().map(|p| {
            SvgPoint {
                x: (p.x - riss.min_x) / riss.width_m() * riss_config.width_mm as f64, 
                y: (p.y - riss.min_y) / riss.height_m() * riss_config.height_mm as f64, 
            }
        }).collect()
    }
}

fn write_rote_linien(
    layer: &mut PdfLayerReference,
    linien: &[SvgLine]
) -> Option<()> {

    layer.save_graphics_state();

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 255.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(1.0);

    for l in linien.iter() {
        layer.add_line(printpdf::Line { 
            points: l.points.iter().map(|p| (printpdf::Point {
                x: Mm(p.x as f32).into_pt(),
                y: Mm(p.y as f32).into_pt(),
            }, false)).collect(), 
            is_closed: l.is_closed() 
        })
    }

    layer.restore_graphics_state();
    
    Some(())

}

fn write_border(
    layer: &mut PdfLayerReference,
    border_width_mm: f32,
    riss: &RissConfig,
) -> Option<()> {

    use printpdf::Point;

    let add_rect = |x, y, w, h, paintmode| {
    
        let points = vec![
            (Point { x: Mm(x).into(), y: Mm(y).into() }, false),
            (Point { x: Mm(x + w).into(), y: Mm(y).into() }, false),
            (Point { x: Mm(x + w).into(), y: Mm(y + h).into() }, false),
            (Point { x: Mm(x).into(), y: Mm(y + h).into() }, false),
        ];

        let poly = printpdf::Polygon {
            rings: vec![points],
            mode: paintmode,
            winding_order: printpdf::path::WindingOrder::NonZero,
        };

        layer.add_polygon(poly);
    };

    layer.save_graphics_state();

    layer.set_fill_color(printpdf::Color::Rgb(Rgb {
        r: 255.0,
        g: 255.0,
        b: 255.0,
        icc_profile: None,
    }));

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(1.0);

    add_rect(0.0, 0.0, riss.width_mm, border_width_mm, PaintMode::Fill);
    add_rect(0.0, 0.0, border_width_mm, riss.height_mm, PaintMode::Fill);
    add_rect(0.0, riss.height_mm - border_width_mm, riss.width_mm, border_width_mm, PaintMode::Fill);
    add_rect(riss.width_mm - border_width_mm, 0.0, border_width_mm, riss.height_mm, PaintMode::Fill);


    add_rect(
        border_width_mm, 
        border_width_mm, 
        riss.width_mm - (border_width_mm * 2.0), 
        riss.height_mm - (border_width_mm * 2.0), 
        PaintMode::Stroke
    );

    add_rect(
        border_width_mm,
        riss.height_mm - border_width_mm - 35.0,
        175.0,
        35.0,
        PaintMode::Fill
    );

    layer.restore_graphics_state();
    Some(())
}

fn write_nutzungsarten(
    layer: &mut PdfLayerReference,
    split_flurstuecke: &SplitNasXml,
    style: &Konfiguration,
    log: &mut Vec<String>,
) -> Option<()> {

    let mut flurstueck_nutzungen_grouped_by_ebene = BTreeMap::new();
    for (flst_id, flst_parts) in split_flurstuecke.flurstuecke_nutzungen.iter() {
        for f in flst_parts.iter() {
            let flst_ebene = match f.attributes.get("AX_Ebene") {
                Some(s) => s,
                None => continue,
            };

            let flst_kuerzel_alt = match f.get_auto_kuerzel(&flst_ebene) {
                Some(s) => s,
                None => continue,
            };

            let flst_style =  style.pdf.nutzungsarten
            .iter()
            .find_map(|(k, v)|{
                if v.kuerzel != flst_kuerzel_alt {
                    None
                } else {
                    Some((k.clone(), v.clone()))
                }
            });

            let (flst_style_id, flst_style) = match flst_style {
                Some(s) => s,
                None => continue,
            };

            flurstueck_nutzungen_grouped_by_ebene.entry(flst_style_id).or_insert_with(|| Vec::new()).push(f);
        }        
    }

    let flurstueck_nutzungen_grouped_by_ebene = style.pdf.layer_ordnung.iter().filter_map(|s| {
        let polys = flurstueck_nutzungen_grouped_by_ebene.get(s)?;
        let style = style.pdf.nutzungsarten.get(s)?;
        Some((style, polys))
    }).collect::<Vec<_>>();

    // log.push(serde_json::to_string(&flurstueck_nutzungen_grouped_by_ebene).unwrap_or_default());

    for (style, polys) in flurstueck_nutzungen_grouped_by_ebene.iter() {

        layer.save_graphics_state();
    
        let mut paintmode = PaintMode::Fill;
        let fill_color = style.fill_color.as_ref()
        .and_then(|s| csscolorparser::parse(&s).ok())
        .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }));

        let outline_color: Option<printpdf::Color> = style.outline_color.as_ref()
        .and_then(|s| csscolorparser::parse(&s).ok())
        .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }));

        let outline_thickness = style.outline_thickness.unwrap_or(1.0);

        if let Some(fc) = fill_color.as_ref() {
            layer.set_fill_color(fc.clone());
        }
        if let Some(oc) = outline_color.as_ref() {
            layer.set_outline_color(oc.clone());
            layer.set_outline_thickness(outline_thickness);
            paintmode = PaintMode::FillStroke;
        }

        for poly in polys.iter() {
            layer.add_polygon(translate_poly(&poly.poly, paintmode));
        }

        layer.restore_graphics_state();
    }
    Some(())
}

pub struct FlurstueckeInPdfSpace {
    pub flst: Vec<TaggedPolygon>,
}

pub fn get_flurstuecke_in_pdf_space(
    xml: &NasXMLFile,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> FlurstueckeInPdfSpace {

    let mut flst = xml.ebenen.get("AX_Flurstueck").cloned().unwrap_or_default();
    flst.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    FlurstueckeInPdfSpace {
        flst: flst.into_iter().map(|s| TaggedPolygon {
            attributes: s.attributes,
            poly: poly_into_pdf_space(&s.poly, riss, riss_config, log)
        }).collect()
    }
}

struct FlurenInPdfSpace {
    pub fluren: Vec<TaggedPolygon>,
}

impl FlurstueckeInPdfSpace {
    pub fn get_fluren(&self, flst: &FlurstueckeInPdfSpace) -> String {
        let mut fluren_map = BTreeMap::new();
        for v in flst.flst.iter() {
            
            let flst = v.attributes
            .get("flurstueckskennzeichen")
            .and_then(|s| FlstIdParsed::from_str(s).parse_num());
    
            let flst = match flst {
                Some(s) => s,
                None => continue,
            };
    
            fluren_map.entry(flst.gemarkung).or_insert_with(|| BTreeSet::new()).insert(flst.flur);
        }


        let mut s = String::new();

        for (gemarkung, flur) in fluren_map {
            s.push_str(&format!("  Gemarkung {gemarkung} Flur {flur:?},  "));
        }
        
        s
        
    }
}

pub fn subtract_from_poly(original: &SvgPolygon, subtract: &[&SvgPolygon]) -> SvgPolygon {
    use geo::BooleanOps;
    let mut first = original.clone();
    for i in subtract.iter() {
        if first.equals(i) {
            continue;
        }
        if first.is_zero_area() {
            return SvgPolygon::default();
        }
        if i.is_zero_area() {
            return SvgPolygon::default();
        }
        if first.equals_any_ring(&i) {
            return first;
        }
        if i.equals_any_ring(&first) {
            return (*i).clone();
        }
        let a = translate_to_geo_poly(&first.round_to_3dec());
        let b = translate_to_geo_poly(&i.round_to_3dec());
        let join = a.difference(&b);
        let s = translate_from_geo_poly(&join);
        let new = SvgPolygon {
            outer_rings: s.iter().flat_map(|s| {
                s.outer_rings.clone().into_iter()
            }).collect(),
            inner_rings: s.iter().flat_map(|s| {
                s.inner_rings.clone().into_iter()
            }).collect(),
        };
        first = new;
    }

    crate::nas::cleanup_poly(&first)
}

pub fn join_polys(polys: &[SvgPolygon]) -> Option<SvgPolygon> {
    use geo::BooleanOps;
    let mut first = match polys.get(0) {
        Some(s) => s.clone(),
        None => return None,
    };
    for i in polys.iter().skip(1) {
        if first.equals(i) {
            continue;
        }
        if i.is_empty() {
            continue;
        }
        let fi = first.round_to_3dec();
        let ii = i.round_to_3dec();
        let a = translate_to_geo_poly(&fi);
        let b = translate_to_geo_poly(&ii);     
        let join = a.union(&b);
        let s = translate_from_geo_poly(&join);
        let new = SvgPolygon {
            outer_rings: s.iter().flat_map(|s| {
                s.outer_rings.clone().into_iter()
            }).collect(),
            inner_rings: s.iter().flat_map(|s| {
                s.inner_rings.clone().into_iter()
            }).collect(),
        };
        first = new;
    }

    Some(crate::nas::cleanup_poly(&first))
}

pub fn difference_polys(polys: &[SvgPolygon]) -> Option<SvgPolygon> {
    use geo::BooleanOps;
    let mut first = match polys.get(0) {
        Some(s) => s.clone(),
        None => return None,
    };
    for i in polys.iter().skip(1) {
        let a = translate_to_geo_poly(&first.round_to_3dec());
        let b = translate_to_geo_poly(&i.round_to_3dec());
        let join = a.difference(&b);
        let s = translate_from_geo_poly(&join);
        let new = SvgPolygon {
            outer_rings: s.iter().flat_map(|s| {
                s.outer_rings.clone().into_iter()
            }).collect(),
            inner_rings: s.iter().flat_map(|s| {
                s.inner_rings.clone().into_iter()
            }).collect(),
        };
        first = new;
    }

    Some(crate::nas::cleanup_poly(&first))
}

fn get_fluren_in_pdf_space(
    flst: &FlurstueckeInPdfSpace,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> FlurenInPdfSpace {

    let mut fluren_map = BTreeMap::new();
    for v in flst.flst.iter() {
        
        let flst = v.attributes
        .get("flurstueckskennzeichen")
        .and_then(|s| FlstIdParsed::from_str(s).parse_num());

        let flst = match flst {
            Some(s) => s,
            None => continue,
        };

        fluren_map.entry(flst.gemarkung).or_insert_with(|| Vec::new()).push(v);
    }


    FlurenInPdfSpace {
        fluren: fluren_map.iter().filter_map(|(k, v)| {
            let polys = v.iter().map(|s| s.poly.clone()).collect::<Vec<_>>();
            let joined = join_polys(&polys)?;
            Some(TaggedPolygon {
                attributes: vec![("berechneteGemarkung".to_string(), k.to_string())].into_iter().collect(),
                poly: joined,
            })
        }).collect()
    }
}

fn write_flurstuecke(
    layer: &mut PdfLayerReference,
    flst: &FlurstueckeInPdfSpace,
    style: &Konfiguration,
    log: &mut Vec<String>,
) -> Option<()> {

    layer.save_graphics_state();

    layer.set_fill_color(printpdf::Color::Rgb(Rgb {
        r: 255.0,
        g: 255.0,
        b: 255.0,
        icc_profile: None,
    }));

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(1.0);

    for tp in flst.flst.iter() {
        let poly = translate_poly(&tp.poly, PaintMode::Stroke);
        layer.add_polygon(poly);
    }

    layer.restore_graphics_state();

    Some(())
}

fn write_fluren(
    layer: &mut PdfLayerReference,
    fluren: &FlurenInPdfSpace,
    style: &Konfiguration,
    log: &mut Vec<String>,
) -> Option<()> {

    layer.save_graphics_state();

    layer.set_fill_color(printpdf::Color::Rgb(Rgb {
        r: 255.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(3.0);

    for tp in fluren.fluren.iter() {
        let poly = translate_poly(&tp.poly, PaintMode::Stroke);
        layer.add_polygon(poly);
    }

    layer.restore_graphics_state();

    Some(())
}

fn translate_poly(
    svg: &SvgPolygon,
    paintmode: PaintMode,
) -> printpdf::Polygon {
    printpdf::Polygon {
        rings: {
            let mut r = Vec::new();
            for outer in svg.outer_rings.iter() {
                let points = outer.points.clone();
                r.push(points.into_iter().map(|p| printpdf::Point {
                    x: Mm(p.x as f32).into_pt(),
                    y: Mm(p.y as f32).into_pt(),
                }).map(|p| (p, false)).collect());
            }
            for inner in svg.inner_rings.iter() {
                let mut points = inner.points.clone();
                points.reverse();
                r.push(points.into_iter().map(|p| printpdf::Point {
                    x: Mm(p.x as f32).into_pt(),
                    y: Mm(p.y as f32).into_pt(),
                }).map(|p| (p, false)).collect());
            }
            r
        },
        mode: paintmode,
        winding_order: printpdf::path::WindingOrder::NonZero,
    }
}