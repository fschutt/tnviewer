use std::collections::BTreeMap;

use printpdf::{CustomPdfConformance, Mm, PdfConformance, PdfDocument, PdfLayerReference};
use serde_derive::{Deserialize, Serialize};
use crate::analyze::LatLng;
use crate::csv::CsvDataType;
use crate::nas::{NasXMLFile, SplitNasXml, SvgLine, SvgPoint, SvgPolygon, TaggedPolygon};
use crate::ui::{Aenderungen, PolyNeu};

pub type Risse = BTreeMap<String, RissConfig>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EbenenStyle {
    #[serde(default)]
    fill_color: Option<String>,
    #[serde(default)]
    outline_color: Option<String>,
    #[serde(default)]
    outline_thickness: Option<f32>,
    #[serde(default)]
    outline_dash: Option<String>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    outline_overprint: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StyleConfig {
    pub nutzungsartengrenze_col: Option<String>,
    pub nutzungsartengrenze_thickness: Option<f32>,
    pub grenzpunkt_svg: Option<String>,
    pub gebauede_loeschen_svg: Option<String>,
    pub ebenen: BTreeMap<String, EbenenStyle>,
}

pub type RissMap = BTreeMap<String, RissExtent>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RissExtent {
    pub coords: Vec<LatLng>,
    pub projection: String,
}

impl RissExtent {
    // latlon -> 
    pub fn reproject(&self, target_crs: &str) -> Option<RissExtentReprojected> {
        
        let coords = self.coords.iter().map(|l| {
            (l.lng.to_radians(), l.lat.to_radians(), 0.0)
        }).collect::<Vec<_>>();
        if coords.is_empty() {
            return None;
        }

        let source = proj4rs::Proj::from_proj_string(&self.projection).ok()?;
        let target = proj4rs::Proj::from_proj_string(&target_crs).ok()?;
        let points = coords.iter().filter_map(|p| {
            let mut p = p.clone();
            proj4rs::transform::transform(&source, &target, &mut p).ok()?;
            Some(SvgPoint {
                x: p.0, 
                y: p.1,
            })
        }).collect::<Vec<_>>();
        let spec = 1000000.0;
        Some(RissExtentReprojected {
            crs: target_crs.to_string(),
            max_x: points.iter().map(|v| (v.x * spec) as usize).max().unwrap_or(0) as f64 / spec,
            min_x: points.iter().map(|v| (v.x * spec) as usize).min().unwrap_or(0) as f64 / spec,
            max_y: points.iter().map(|v| (v.x * spec) as usize).max().unwrap_or(0) as f64 / spec,
            min_y: points.iter().map(|v| (v.x * spec) as usize).min().unwrap_or(0) as f64 / spec,
        })

    }
}

#[derive(Debug, Clone)]
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
    style: &StyleConfig,
    csv: &CsvDataType, 
    xml: &NasXMLFile,
    split_flurstuecke: &SplitNasXml,
    aenderungen: &Aenderungen, 
    risse: &Risse,
    riss_map: &RissMap,
    log: &mut Vec<String>
) -> Vec<u8> {

    let (mut doc, page1, layer1) = PdfDocument::new(
        "Riss",
        Mm(risse.iter().next().map(|(k, v)| v.width_mm).unwrap_or(210.0)),
        Mm(risse.iter().next().map(|(k, v)| v.height_mm).unwrap_or(297.0)),
        "Riss",
    );

    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        .. Default::default()
    }));

    for (i, (ri, rc))  in risse.iter().enumerate() {

        log.push(format!("Rendering Riss {ri}"));

        let riss_extent = match riss_map.get(ri).and_then(|r| r.reproject(&xml.crs)) {
            Some(s) => s,
            None => continue,
        };

        let (page, layer) = if i == 0 {
            (page1, layer1)
        } else {
            doc.add_page(Mm(rc.width_mm), Mm(rc.height_mm), ri)
        };

        let mut page = doc.get_page(page);
        let mut layer = page.get_layer(layer);

        let aenderungen_in_pdf_space = match reproject_aenderungen_into_pdf_space(
            &aenderungen,
            &riss_extent,
            rc,
            &xml.crs,
            log,
        ) {
            Ok(o) => o,
            Err(_) => continue,
        };

        log.push(format!("aenderungen ok---"));

        log.push(format!("Rendering Riss {ri}: 2 ok"));

        let split_flurstuecke_in_pdf_space = reproject_splitnas_into_pdf_space(
            &split_flurstuecke,
            &riss_extent,
            rc,
            log
        );

        log.push(format!("Rendering Riss {ri}: 3 ok"));

        // log.push(serde_json::to_string(&split_flurstuecke_in_pdf_space).unwrap_or_default());

        write_split_flurstuecke_into_layer(&mut layer, &split_flurstuecke_in_pdf_space, &style, log);

        let nas_xml_in_pdf_space = reproject_nasxml_into_pdf_space(
            &xml,
            &riss_extent,
            rc,
            log,
        );

        // log.push(format!("nas xml in pdf space {}", serde_json::to_string(&nas_xml_in_pdf_space).unwrap_or_default()));

        log.push(format!("Rendering Riss {ri}: 4 ok"));
    }

    log.push(format!("Rendering Risse: 5 ok"));

    doc.save_to_bytes().unwrap_or_default()
}

#[inline(always)]
fn reproject_aenderungen_into_pdf_space(
    aenderungen: &Aenderungen,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    original_crs: &str,
    log: &mut Vec<String>
) -> Result<Aenderungen, String> {
    use crate::nas::LATLON_STRING;

    let target_proj = proj4rs::Proj::from_proj_string(&original_crs)
    .map_err(|e| format!("source_proj_string: {e}: {:?}", original_crs))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
    .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    let target_riss = quadtree_f32::Rect {
        min_x: riss.min_x,
        min_y: riss.min_y,
        max_x: riss.max_x,
        max_y: riss.max_y,
    };
    Ok(Aenderungen {
        gebaeude_loeschen: aenderungen.gebaeude_loeschen.clone(),
        na_definiert: aenderungen.na_definiert.clone(),
        na_polygone_neu: aenderungen.na_polygone_neu
        .iter()
        .map(|(k, v)| {
            (k.clone(), PolyNeu {
                poly: crate::nas::reproject_poly(&v.poly, &latlon_proj, &target_proj),
                nutzung: v.nutzung.clone(),
            })
        })
        .filter_map(|(k, v)| {
            if v.poly.get_rect().overlaps_rect(&target_riss) {
                Some((k.clone(), PolyNeu {
                    poly: poly_into_pdf_space(&v.poly, &riss, riss_config, log),
                    nutzung: v.nutzung,
                }))
            } else {
                None
            }
        })
        .collect()
    })
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
        flurstuecke_nutzungen: split_flurstuecke.flurstuecke_nutzungen.iter().map(|(k, v)| {
            (k.clone(), v.iter().filter_map(|s| {
                if s.get_rect().overlaps_rect(&target_riss) {
                    Some(TaggedPolygon {
                        attributes: s.attributes.clone(),
                        poly: poly_into_pdf_space(&s.poly, &riss, riss_config, log),
                    })
                } else {
                    None
                }
            }).collect())
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
    log.push(serde_json::to_string(line).unwrap_or_default());
    let l = SvgLine {
        points: line.points.iter().map(|p| {
            SvgPoint {
                x: riss_config.width_mm as f64 / riss.width_m() * p.x, 
                y: riss_config.height_mm as f64 - (riss_config.height_mm as f64 / riss.height_m() * p.y), 
            }
        }).collect()
    };

    log.push(serde_json::to_string(&l).unwrap_or_default());
    l
}

fn write_split_flurstuecke_into_layer(
    layer: &mut PdfLayerReference,
    split_flurstuecke: &SplitNasXml,
    style: &StyleConfig,
    log: &mut Vec<String>,
) -> Option<()> {
    // let flurstuecke_nutzungen_in_riss = write_split_flurstuecke_into_layer
    for (k, v) in split_flurstuecke.flurstuecke_nutzungen.iter() {
        let style = match style.ebenen.get(k) {
            Some(s) => s,
            None => continue,
        };
        for poly in v.iter() {
            let pdf_poly = translate_poly(&poly.poly);
            log.push(format!("drawing polygon {k}: {pdf_poly:#?}"));
            layer.add_polygon(pdf_poly);
        }
    }
    Some(())
}

fn translate_poly(
    svg: &SvgPolygon,
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
        mode: printpdf::path::PaintMode::Fill,
        winding_order: printpdf::path::WindingOrder::NonZero,
    }
}