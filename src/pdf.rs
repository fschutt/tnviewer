use std::collections::{BTreeMap, BTreeSet};

use printpdf::path::PaintMode;
use printpdf::{CustomPdfConformance, IndirectFontRef, Mm, PdfConformance, PdfDocument, PdfLayerReference, Rgb, TextRenderingMode};
use quadtree_f32::QuadTree;
use serde_derive::{Deserialize, Serialize};
use web_sys::console::log_1;
use crate::geograf::{get_aenderungen_rote_linien, LinienQuadTree};
use crate::{nas, LatLng};
use crate::csv::CsvDataType;
use crate::nas::{
    intersect_polys, parse_nas_xml, translate_from_geo_poly, translate_to_geo_poly, NasXMLFile, SplitNasXml, SvgLine, SvgPoint, SvgPolygon, TaggedPolygon, UseRadians, LATLON_STRING
};
use crate::ui::{Aenderungen, AenderungenIntersection, PolyNeu, TextPlacement};
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

impl PdfEbenenStyle {
    pub fn default_grau(kuerzel: &str) -> Self {
        PdfEbenenStyle {
            kuerzel: kuerzel.to_string(),
            fill_color: None,
            fill: false,
            outline_color: Some("#6082B6".to_string()),
            outline_thickness: Some(0.1),
            outline_overprint: false,
            outline_dash: None,
            pattern_svg: None,
            pattern_placement: None,
            lagebez_ohne_hsnr: PtoStil::default(),
        }
    }
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
    pub fn get_poly(&self) -> SvgPolygon {
        let mut v = vec![
            SvgPoint { x: self.min_x, y: self.min_y },
            SvgPoint { x: self.min_x, y: self.max_y },
            SvgPoint { x: self.max_x, y: self.max_y },
            SvgPoint { x: self.max_x, y: self.min_y },
            SvgPoint { x: self.min_x, y: self.min_y },
        ];
        v.reverse();
        SvgPolygon { outer_rings: vec![
            SvgLine {
                points: v
            }
        ], inner_rings: Vec::new() }
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

    let lq = nas_cut_original.get_linien_quadtree();

    generate_pdf_internal(
        projekt_info,
        konfiguration,
        &xml,
        &nas_cut_original,
        &[],
        risse,
        &riss_map.iter().filter_map(|(k, v)| Some((k.clone(), v.reproject(&xml.crs, log)?))).collect(),
        log,
        &lq,
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
    log: &mut Vec<String>,
    linienquadtree: &LinienQuadTree,
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

    let helvetica = match doc.add_builtin_font(printpdf::BuiltinFont::HelveticaBold) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let times_roman = match doc.add_builtin_font(printpdf::BuiltinFont::TimesRoman) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let times_roman_bold = match doc.add_builtin_font(printpdf::BuiltinFont::TimesBold) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

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

        let gebaeude = get_gebaeude_in_pdf_space(&xml, riss_extent, rc, log);

        let _ = write_gebaeude(&mut layer, &gebaeude, log);

        let flst = get_flurstuecke_in_pdf_space(
            &xml,
            &riss_extent,
            rc,
            log
        );

        let _ = write_flurstuecke(&mut layer, &flst, &konfiguration, log);

        // let _ = write_grenzpunkte(&mut layer, &flst, &konfiguration, log);

        let fluren = get_fluren_in_pdf_space(
            &xml,
            &riss_extent,
            rc,
            log
        );
        
        let _ = write_fluren(&mut layer, &fluren, &konfiguration, log);
        
        let rote_linien = get_aenderungen_rote_linien(splitflaechen, linienquadtree)
        .into_iter().map(|l| {
            line_into_pdf_space(&l, riss_extent, rc, &mut Vec::new())
        }).collect::<Vec<_>>();

        let _ = write_rote_linien(&mut layer, &rote_linien);
        
        let _ = write_splitflaechen_beschriftungen(
            &mut layer, 
            &helvetica,
            splitflaechen, 
            riss_extent, 
            rc
        );

        let _ = write_border(
            &mut layer, 
            &rc,
            projekt_info,
            nas_cut_original,
            &times_roman,
            &times_roman_bold,
            Some(riss_extent.get_rect()),
            i + 1,
            risse.len(),
            16.5
        );
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
            point_into_pdf_space(p, riss, riss_config)
        }).collect()
    }
}

fn point_into_pdf_space(
    p: &SvgPoint,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig
) -> SvgPoint {
    SvgPoint {
        x: (p.x - riss.min_x) / riss.width_m() * riss_config.width_mm as f64, 
        y: (p.y - riss.min_y) / riss.height_m() * riss_config.height_mm as f64, 
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

fn write_splitflaechen_beschriftungen(
    layer: &mut PdfLayerReference,
    font: &IndirectFontRef,
    splitflaechen: &[AenderungenIntersection],
    riss_extent: &RissExtentReprojected,
    riss: &RissConfig,
) -> Option<()> {

    let riss_poly = riss_extent.get_poly();

    for sf in splitflaechen.iter() {
        log_1(&format!("PDF splitflaeche: {sf:?}").into());
    }

    log_1(&format!("RISS poly: {riss_poly:?}").into());

    let splitflaechen = splitflaechen.iter().flat_map(|sf| {
        intersect_polys(&sf.poly_cut, &riss_poly, true, true)
        .into_iter()
        .map(|f| {
            AenderungenIntersection {
                alt: sf.alt.clone(),
                neu: sf.neu.clone(),
                flst_id: sf.flst_id.clone(),
                poly_cut: f.round_to_3dec(),
            }
        })
    }).collect::<Vec<_>>();
    
    log_1(&format!("sf1").into());

    let texte_bleibt = splitflaechen.iter()
    .filter_map(|s| s.get_text_bleibt())
    .map(|p| {
        TextPlacement {
            kuerzel: p.kuerzel,
            status: p.status,
            pos: point_into_pdf_space(&p.pos, riss_extent, riss),
        }
    })
    .collect::<Vec<_>>();

    log_1(&format!("sf2").into());

    let texte_neu = splitflaechen.iter()
    .filter_map(|s| s.get_text_neu())
    .map(|p| {
        TextPlacement {
            kuerzel: p.kuerzel,
            status: p.status,
            pos: point_into_pdf_space(&p.pos, riss_extent, riss),
        }
    })
    .collect::<Vec<_>>();

    log_1(&format!("sf4").into());

    let texte_alt = splitflaechen.iter()
    .filter_map(|s| s.get_text_alt())
    .map(|p| {
        TextPlacement {
            kuerzel: p.kuerzel,
            status: p.status,
            pos: point_into_pdf_space(&p.pos, riss_extent, riss),
        }
    })
    .collect::<Vec<_>>();


    for l in texte_alt.iter() {
        log_1(&format!("TEXT ALT: {l:?}").into());
    }

    for l in texte_neu.iter() {
        log_1(&format!("TEXT NEU: {l:?}").into());
    }

    for l in texte_bleibt.iter() {
        log_1(&format!("TEXT BLEIBT: {l:?}").into());
    }

    log_1(&format!("PDF TEXTE: {} bleibt {} alt {} neu", texte_bleibt.len(), texte_alt.len(), texte_neu.len()).into());

    let alt_color = csscolorparser::parse("#cc0000").ok()
    .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }))
    .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let neu_color = csscolorparser::parse("#00aa00").ok()
    .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }))
    .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let bleibt_color = csscolorparser::parse("#6082B6").ok()
    .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }))
    .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.save_graphics_state();
    

    layer.set_fill_color(bleibt_color);
    for t in texte_bleibt {
        layer.begin_text_section();
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(t.pos.x as f32), Mm(t.pos.y as f32));
        layer.write_text(t.kuerzel, &font);
        layer.end_text_section();
    }

    layer.set_fill_color(alt_color);
    for t in texte_alt {
        layer.begin_text_section();
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(t.pos.x as f32), Mm(t.pos.y as f32));
        layer.write_text(t.kuerzel, &font);
        layer.end_text_section();
    }

    layer.set_fill_color(neu_color);
    for t in texte_neu {
        layer.begin_text_section();
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(t.pos.x as f32), Mm(t.pos.y as f32));
        layer.write_text(t.kuerzel, &font);
        layer.end_text_section();
    }

    layer.restore_graphics_state();

    Some(())
}

fn write_border(
    layer: &mut PdfLayerReference,
    riss: &RissConfig,
    info: &ProjektInfo,
    split_nas: &SplitNasXml,
    times_roman: &IndirectFontRef,
    times_roman_bold: &IndirectFontRef,
    extent_rect: Option<quadtree_f32::Rect>,
    num_riss: usize,
    total_risse: usize,
    border_width_mm: f32,
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

    let _ = crate::geograf::write_header(
        layer,
        info,
        split_nas,
        times_roman,
        times_roman_bold,
        extent_rect,
        num_riss,
        total_risse,
        riss.height_mm - border_width_mm - 35.0,
        border_width_mm,
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
    
    let mut flurstueck_nutzungen_grouped_by_ebene = Vec::new();

    if style.pdf.nutzungsarten.is_empty() {
        flurstueck_nutzungen_grouped_by_ebene = split_flurstuecke.flurstuecke_nutzungen.iter().map(|(f, v)| {
            (PdfEbenenStyle::default_grau(f), v.iter().collect::<Vec<_>>())
        }).collect();
    } else {

        let mut fl_btree = BTreeMap::new();
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
    
                fl_btree.entry(flst_style_id).or_insert_with(|| Vec::new()).push(f);
            }        
        }
    
        flurstueck_nutzungen_grouped_by_ebene = style.pdf.layer_ordnung.iter().filter_map(|s| {
            let polys = fl_btree.get(s)?.clone();
            let style = style.pdf.nutzungsarten.get(s)?.clone();
            Some((style, polys))
        }).collect::<Vec<_>>();    
    }

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
            paintmode = if fill_color.is_some() {
                PaintMode::FillStroke
            } else {
                PaintMode::Stroke
            };
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

struct GebaeudeInPdfSpace {
    pub gebaeude: Vec<TaggedPolygon>,
}

struct FlurenInPdfSpace {
    pub fluren: Vec<TaggedPolygon>,
}

// only called in stage6 + get_intersections!
pub fn subtract_from_poly(original: &SvgPolygon, subtract: &[&SvgPolygon]) -> SvgPolygon {
    use geo::BooleanOps;
    let mut first = original.round_to_3dec();
    for i in subtract.iter() {
        let fi = first.round_to_3dec();
        let mut i = i.round_to_3dec();
        if fi.equals(&i) {
            continue;
        }
        log_1(&"correcting almost touching points...".into());
        i.correct_almost_touching_points(&fi, 0.05, true);
        let i = i.round_to_3dec();
        log_1(&"ok subtract!".into());
        log_1(&serde_json::to_string(&fi).unwrap_or_default().into());
        log_1(&serde_json::to_string(&i).unwrap_or_default().into());
        if fi.is_zero_area() {
            return SvgPolygon::default();
        }
        if i.is_zero_area() {
            return SvgPolygon::default();
        }
        if fi.equals_any_ring(&i).is_some() {
            return fi;
        }
        if i.equals_any_ring(&fi).is_some() {
            return i;
        }
        let relate = nas::relate(&fi, &i);
        if relate.only_touches() {
            continue;
        }
        if relate.b_contained_in_a() {
            first = i;
            continue;
        }
        let a = translate_to_geo_poly(&fi);
        let b = translate_to_geo_poly(&i);
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
        first = crate::nas::cleanup_poly(&new);
    }

    first
}

pub fn join_polys(polys: &[SvgPolygon], autoclean: bool) -> Option<SvgPolygon> {
    use geo::BooleanOps;
    let mut first = match polys.get(0) {
        Some(s) => s.round_to_3dec(),
        None => return None,
    };
    for i in polys.iter().skip(1) {
        let i = i.round_to_3dec();
        if first.equals(&i) {
            continue;
        }
        if i.is_empty() {
            continue;
        }
        let fi = first.round_to_3dec();
        let a = translate_to_geo_poly(&fi);
        let b = translate_to_geo_poly(&i);     
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
        if autoclean {
            first = crate::nas::cleanup_poly(&new);
        } else {
            first = new;
        }
    }

    Some(first)
}

fn get_gebaeude_in_pdf_space(
    xml: &NasXMLFile,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> GebaeudeInPdfSpace {

    let mut gebaeude = xml.ebenen.get("AX_Gebaeude").cloned().unwrap_or_default();
    gebaeude.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    GebaeudeInPdfSpace {
        gebaeude: gebaeude.iter().filter_map(|v| {
            let joined = poly_into_pdf_space(&v.poly, riss, riss_config, log);
            Some(TaggedPolygon {
                attributes: BTreeMap::new(),
                poly: joined,
            })
        }).collect()
    }
}

fn get_fluren_in_pdf_space(
    xml: &NasXMLFile,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    log: &mut Vec<String>
) -> FlurenInPdfSpace {

    let mut flst = xml.ebenen.get("AX_Flurstueck").cloned().unwrap_or_default();
    flst.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    let mut fluren_map = BTreeMap::new();
    for v in flst.iter() {
        
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
            let polys = v.iter()
            .map(|s| s.poly.clone())
            .collect::<Vec<_>>();
            let joined = join_polys(&polys, false)?;
            let joined = poly_into_pdf_space(&joined, riss, riss_config, log);
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

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(0.1);

    for tp in flst.flst.iter() {
        let poly = translate_poly(&tp.poly, PaintMode::Stroke);
        layer.add_polygon(poly);
    }

    layer.restore_graphics_state();

    Some(())
}


fn write_gebaeude(
    layer: &mut PdfLayerReference,
    gebaeude: &GebaeudeInPdfSpace,
    log: &mut Vec<String>,
) -> Option<()> {

    let fill_color = csscolorparser::parse("#808080").ok()
    .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }))
    .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let outline_color = csscolorparser::parse("#000000").ok()
    .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }))
    .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.save_graphics_state();

    layer.set_fill_color(fill_color);

    layer.set_outline_color(outline_color);

    layer.set_outline_thickness(0.1);

    for tp in gebaeude.gebaeude.iter() {
        let poly = translate_poly(&tp.poly, PaintMode::FillStroke);
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

    let outline_color = csscolorparser::parse("#F8196F").ok()
    .map(|c| printpdf::Color::Rgb(printpdf::Rgb { r: c.r as f32, g: c.g as f32, b: c.b as f32, icc_profile: None }))
    .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.set_overprint_stroke(true);

    layer.set_outline_color(outline_color);

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