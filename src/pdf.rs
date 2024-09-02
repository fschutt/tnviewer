use std::collections::{BTreeMap, BTreeSet};
use std::io::Split;

use printpdf::path::PaintMode;
use printpdf::{CustomPdfConformance, IndirectFontRef, Mm, PdfConformance, PdfDocument, PdfLayerReference, Rgb, TextRenderingMode};
use quadtree_f32::QuadTree;
use serde_derive::{Deserialize, Serialize};
use web_sys::console::log_1;
use crate::geograf::{get_aenderungen_rote_linien, HeaderCalcConfig, LinienQuadTree};
use crate::optimize::{OptimizeConfig, OptimizedTextPlacement};
use crate::uuid_wasm::log_status;
use crate::{nas, LatLng};
use crate::csv::CsvDataType;
use crate::nas::{
    translate_from_geo_poly, translate_to_geo_poly, EqualsAnyRingStatus, NasXMLFile, SplitNasXml, SvgLine, SvgPoint, SvgPolygon, TaggedPolygon, UseRadians, LATLON_STRING
};
use crate::ui::{Aenderungen, AenderungenIntersection, PolyNeu, TextPlacement, TextStatus};
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

pub struct Fluren {
    pub fluren: Vec<TaggedPolygon>,
}

pub struct FlurenInPdfSpace {
    pub fluren: Vec<TaggedPolygon>,
}


impl Fluren {
    pub fn to_pdf_space(&self, riss: &RissExtentReprojected, rc: &RissConfig) -> FlurenInPdfSpace {
        FlurenInPdfSpace {
            fluren: self.fluren.iter().map(|tp| TaggedPolygon {
                attributes: tp.attributes.clone(),
                poly: poly_into_pdf_space(&tp.poly, riss, rc)
            }).collect()
        }
    }
}


pub struct Flurstuecke {
    pub flst: Vec<TaggedPolygon>,
}

pub struct FlurstueckeInPdfSpace {
    pub flst: Vec<TaggedPolygon>,
}

impl Flurstuecke {
    pub fn to_pdf_space(&self, riss: &RissExtentReprojected, rc: &RissConfig) -> FlurstueckeInPdfSpace {
        FlurstueckeInPdfSpace {
            flst: self.flst.iter().map(|tp| TaggedPolygon {
                attributes: tp.attributes.clone(),
                poly: poly_into_pdf_space(&tp.poly, riss, rc)
            }).collect()
        }
    }
}

pub struct Gebaeude {
    pub gebaeude: Vec<TaggedPolygon>,
}

pub struct GebaeudeInPdfSpace {
    pub gebaeude: Vec<TaggedPolygon>,
}

impl Gebaeude {
    pub fn to_pdf_space(&self, riss: &RissExtentReprojected, rc: &RissConfig) -> GebaeudeInPdfSpace {
        GebaeudeInPdfSpace {
            gebaeude: self.gebaeude.iter().map(|tp| TaggedPolygon {
                attributes: tp.attributes.clone(),
                poly: poly_into_pdf_space(&tp.poly, riss, rc)
            }).collect()
        }
    }
}

pub fn generate_pdf_internal(
    riss_von: (usize, usize), // Riss X von Y
    projekt_info: &ProjektInfo,
    calc: &HeaderCalcConfig,
    konfiguration: &Konfiguration,
    nutzungsarten: &SplitNasXml,
    rc: &RissConfig,
    riss_extent: &RissExtentReprojected,

    splitflaechen: &[AenderungenIntersection],
    rote_linien: &Vec<SvgLine>, // in ETRS space
    beschriftungen: &[TextPlacement], // in ETRS space
    fluren: &Fluren, // in ETRS space,
    flst: &Flurstuecke, // in ETRS space
    split_nas_mini: &SplitNasXml,
    gebaeude: &Gebaeude, // in ETRS space
) -> Vec<u8> {

    let (mut doc, page1, layer1) = PdfDocument::new(
        "Riss",
        Mm(rc.width_mm),
        Mm(rc.height_mm),
        &format!("Riss {} / {}", riss_von.0, riss_von.1),
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

    let (page, layer) = (page1, layer1);

    let page = doc.get_page(page);
    let mut layer = page.get_layer(layer);

    let nutzungsarten = reproject_splitnas_into_pdf_space(
        &nutzungsarten,
        &riss_extent,
        rc,
        &mut Vec::new(),
    );

    let _ = write_nutzungsarten(&mut layer, &nutzungsarten, &konfiguration, &mut Vec::new());

    log_status(&format!("Rendere Gebäude..."));
    let _ = write_gebaeude(&mut layer, &gebaeude.to_pdf_space(riss_extent, rc), &mut Vec::new());

    log_status(&format!("Rendere Flurstücke..."));
    let _ = write_flurstuecke(&mut layer, &flst.to_pdf_space(riss_extent, rc), &konfiguration, &mut Vec::new());

    log_status(&format!("Rendere Fluren..."));
    let _ = write_fluren(&mut layer, &fluren.to_pdf_space(riss_extent, rc), &konfiguration, &mut Vec::new());

    log_status(&format!("Rendere rote Linien..."));
    let rote_linien = rote_linien.iter().map(|l| line_into_pdf_space(&l, riss_extent, rc)).collect::<Vec<_>>();
    let _ = write_rote_linien(&mut layer, &rote_linien);

    log_status(&format!("Optimiere Beschriftungen... {:?}", riss_von));
    let aenderungen_texte = crate::optimize::optimize_labels(
        &split_nas_mini,
        splitflaechen,
        &gebaeude,
        &[],
        &beschriftungen,
        &OptimizeConfig::new(rc, riss_extent, 0.5 /* mm */) ,
    );

    log_status(&format!("Rendere Beschriftungen..."));
    let _ = write_splitflaechen_beschriftungen(
        &mut layer, 
        &helvetica,
        riss_extent, 
        rc,
        &aenderungen_texte,
    );

    let _ = write_border(
        &mut layer, 
        &rc,
        projekt_info,
        calc,
        &times_roman,
        &times_roman_bold,
        riss_von.0,
        riss_von.1,
        16.5
    );

    log_status(&format!("PDF fertig."));

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
        flst_id_part: s.flst_id_part.clone(),
        poly_cut: poly_into_pdf_space(&s.poly_cut, &riss, riss_config),
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
                        poly: poly_into_pdf_space(&s.poly, &riss, riss_config),
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
                    poly: poly_into_pdf_space(&s.poly, &riss, riss_config),
                }
            }).collect())
        }).collect()
    }
}

fn poly_into_pdf_space(
    poly: &SvgPolygon,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
) -> SvgPolygon {
    SvgPolygon { 
        outer_rings: poly.outer_rings.iter().map(|l| line_into_pdf_space(l, riss, riss_config)).collect(), 
        inner_rings: poly.inner_rings.iter().map(|l| line_into_pdf_space(l, riss, riss_config)).collect(), 
    }
}

fn line_into_pdf_space(
    line: &SvgLine,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
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
    linien: &[SvgLine],
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
    riss_extent: &RissExtentReprojected,
    riss: &RissConfig,
    beschriftungen: &[OptimizedTextPlacement],
) -> Option<()> {

    let linien = beschriftungen.iter().filter_map(|l| {
        let (start, end) = l.get_line()?;
        let start = point_into_pdf_space(&start, riss_extent, riss);
        let end = point_into_pdf_space(&end, riss_extent, riss);

        Some((l.optimized.status.clone(), printpdf::Line {
            points: vec![
                (printpdf::Point { x: Mm(start.x as f32).into_pt(), y: Mm(start.y as f32).into_pt() }, false),
                (printpdf::Point { x: Mm(end.x as f32).into_pt(), y: Mm(end.y as f32).into_pt() }, false),
            ],
            is_closed: false,
        }))
    }).collect::<Vec<_>>();

    let texte_alt = beschriftungen.iter()
    .filter(|s| s.optimized.status == TextStatus::Old)
    .map(|p| {
        TextPlacement {
            kuerzel: p.optimized.kuerzel.clone(),
            status: p.optimized.status.clone(),
            pos: point_into_pdf_space(&p.optimized.pos, riss_extent, riss),
            area: p.optimized.area,
            poly: p.optimized.poly.clone(),
        }
    })
    .collect::<Vec<_>>();


    let texte_neu = beschriftungen.into_iter()
    .filter(|s| s.optimized.status == TextStatus::New)
    .map(|p| {
        TextPlacement {
            kuerzel: p.optimized.kuerzel.clone(),
            status: p.optimized.status.clone(),
            pos: point_into_pdf_space(&p.optimized.pos, riss_extent, riss),
            area: p.optimized.area,
            poly: p.optimized.poly.clone(),
        }
    })
    .collect::<Vec<_>>();

    let texte_bleibt = beschriftungen.into_iter()
    .filter(|s| s.optimized.status == TextStatus::StaysAsIs)
    .map(|p| {
        TextPlacement {
            kuerzel: p.optimized.kuerzel.clone(),
            status: p.optimized.status.clone(),
            pos: point_into_pdf_space(&p.optimized.pos, riss_extent, riss),
            area: p.optimized.area,
            poly: p.optimized.poly.clone(),
        }
    })
    .collect::<Vec<_>>();

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
    
    layer.set_fill_color(bleibt_color.clone());
    for t in texte_bleibt {
        layer.begin_text_section();
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(t.pos.x as f32), Mm(t.pos.y as f32));
        layer.write_text(t.kuerzel, &font);
        layer.end_text_section();
    }

    layer.set_fill_color(alt_color.clone());
    for t in texte_alt {
        layer.begin_text_section();
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(t.pos.x as f32), Mm(t.pos.y as f32));
        layer.write_text(t.kuerzel, &font);
        layer.end_text_section();
    }

    layer.set_fill_color(neu_color.clone());
    for t in texte_neu {
        layer.begin_text_section();
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(t.pos.x as f32), Mm(t.pos.y as f32));
        layer.write_text(t.kuerzel, &font);
        layer.end_text_section();
    }

    layer.restore_graphics_state();

    layer.save_graphics_state();

    layer.set_outline_thickness(1.0);

    for (ts, li) in linien.iter() {
        let col = match ts {
            TextStatus::New => neu_color.clone(),
            TextStatus::StaysAsIs => bleibt_color.clone(),
            TextStatus::Old => alt_color.clone(),
        };
        layer.set_outline_color(col);
        layer.add_line(li.clone());
    }

    layer.restore_graphics_state();
    

    Some(())
}

fn write_border(
    layer: &mut PdfLayerReference,
    riss: &RissConfig,
    info: &ProjektInfo,
    calc: &HeaderCalcConfig,
    times_roman: &IndirectFontRef,
    times_roman_bold: &IndirectFontRef,
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
        calc,
        times_roman,
        times_roman_bold,
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


pub fn get_mini_nas_xml(
    xml: &SplitNasXml,
    riss: &RissExtentReprojected,
) -> SplitNasXml {

    let rb = riss.get_rect();

    SplitNasXml {
        crs: xml.crs.clone(),
        flurstuecke_nutzungen: xml.flurstuecke_nutzungen
        .iter()
        .map(|(k, v)| {
            let mut v = v.clone();
            v.retain(|s| {
                rb.overlaps_rect(&s.get_rect())
            });
            (k.clone(), v)
        }).collect()
    }
}

pub fn get_flurstuecke(
    xml: &NasXMLFile,
    riss: &RissExtentReprojected,
) -> Flurstuecke {

    let mut flst = xml.ebenen.get("AX_Flurstueck").cloned().unwrap_or_default();
    flst.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    Flurstuecke { flst }
}
 
// only called in stage5 (subtracting overlapping Aenderungen)
pub fn subtract_from_poly(original: &SvgPolygon, subtract: &[&SvgPolygon]) -> SvgPolygon {
    use geo::BooleanOps;
    let mut first = original.round_to_3dec();
    for i in subtract.iter() {
        let mut fi = first.round_to_3dec();
        let mut i = i.round_to_3dec();
        i.correct_winding_order();
        fi.correct_winding_order();
        if fi.equals(&i) {
            continue;
        }
        i.correct_almost_touching_points(&fi, 0.05, true);
        let i = i.round_to_3dec();
        if i.is_zero_area() {
            continue;
        }
        if fi.is_zero_area() {
            return SvgPolygon::default();
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
        first = new;
    }

    first.correct_winding_order();
    first
}

pub fn join_polys(polys: &[SvgPolygon], autoclean: bool, debug: bool) -> Option<SvgPolygon> {
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
        let mut fi = first.round_to_3dec();
        fi.correct_winding_order();
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
        first = new;
    }

    first.correct_winding_order();
    Some(first)
}

fn join_poly_only_touches(
    a: &SvgPolygon,
    b: &SvgPolygon
) -> SvgPolygon {
    let mut outer_rings = a.outer_rings.clone();
    let mut inner_rings = a.inner_rings.clone();
    outer_rings.extend(b.outer_rings.iter().cloned());
    inner_rings.extend(b.inner_rings.iter().cloned());
    SvgPolygon { outer_rings, inner_rings }
}

pub fn get_gebaeude(
    xml: &NasXMLFile,
    riss: &RissExtentReprojected,
) -> Gebaeude {

    let mut gebaeude = xml.ebenen.get("AX_Gebaeude").cloned().unwrap_or_default();
    gebaeude.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    Gebaeude { gebaeude }
}

pub fn get_fluren(xml: &NasXMLFile, riss: &RissExtentReprojected) -> Fluren {

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

    Fluren {
        fluren: fluren_map.iter().filter_map(|(k, v)| {
            let polys = v.iter()
            .map(|s| s.poly.clone())
            .collect::<Vec<_>>();
            let joined = join_polys(&polys, false, true)?;
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