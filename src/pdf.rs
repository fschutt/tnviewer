use crate::{
    csv::CsvDataType,
    geograf::{
        get_default_riss_extent,
        HeaderCalcConfig,
        PADDING,
    },
    ops::intersect_polys,
    nas::{
        reproject_poly,
        NasXMLFile,
        SplitNasXml,
        SvgLine,
        SvgPoint,
        SvgPolygon,
        SvgPolygonInner,
        TaggedPolygon,
        UseRadians,
        LATLON_STRING,
    },
    optimize::{
        OptimizeConfig,
        OptimizedTextPlacement,
    },
    process::AngleDegrees,
    ui::{
        Aenderungen,
        AenderungenIntersection,
        PolyNeu,
        TextPlacement,
        TextStatus,
    },
    uuid_wasm::log_status,
    xlsx::FlstIdParsed,
    LatLng,
};
use printpdf::{
    calculate_points_for_circle,
    path::PaintMode,
    CustomPdfConformance,
    ImageTransform,
    IndirectFontRef,
    Mm,
    PdfConformance,
    PdfDocument,
    PdfLayerReference,
    Rgb,
    TextRenderingMode,
};
use serde_derive::{
    Deserialize,
    Serialize,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
};

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

fn one() -> f64 {
    1.0
}
fn zero_point_two() -> f64 {
    0.2
}
fn five() -> f64 {
    5.0
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MapKonfiguration {
    #[serde(default)]
    pub basemap: Option<String>,
    #[serde(default)]
    pub dop_source: Option<String>,
    #[serde(default)]
    pub dop_layers: Option<String>,
    #[serde(default)]
    pub dgm_source: Option<String>,
    #[serde(default)]
    pub dgm_layers: Option<String>,
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
        self.ebenen_ordnung
            .iter()
            .filter_map(|s| self.ebenen.get(s).cloned().map(|q| (s.clone(), q)))
            .collect()
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
        self.layer_ordnung
            .iter()
            .filter_map(|s| self.nutzungsarten.get(s).cloned().map(|q| (s.clone(), q)))
            .collect()
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
    pub fn default_grau(kuerzel: &str, has_background: bool) -> Self {
        PdfEbenenStyle {
            kuerzel: kuerzel.to_string(),
            fill_color: None,
            fill: false,
            outline_color: Some(if has_background { "#ff0000" } else { "#6082B6" }.to_string()),
            outline_thickness: Some(0.1),
            outline_overprint: false,
            outline_dash: None,
            pattern_svg: None,
            pattern_placement: None,
            lagebez_ohne_hsnr: PtoStil::default(),
        }
    }
}

fn default_fill() -> bool {
    true
}

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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RissExtent {
    pub coords: Vec<LatLng>,
    pub scale: f64,
    pub projection: String,
    pub rissgebiet: Option<SvgPolygon>,
}

impl RissExtent {
    // latlon ->
    pub fn reproject(&self, target_crs: &str) -> Option<RissExtentReprojected> {
        let mut coords = self
            .coords
            .iter()
            .map(|l| (l.lng.to_radians(), l.lat.to_radians(), 0.0))
            .collect::<Vec<_>>();
        if coords.is_empty() {
            return None;
        }

        let source = proj4rs::Proj::from_proj_string(LATLON_STRING).ok()?;
        let target = proj4rs::Proj::from_proj_string(&target_crs).ok()?;
        let rissgebiet = self.rissgebiet.as_ref().map(|s| {
            let i = s.get_inner();
            let already_reprojected = i.outer_ring.points.iter().any(|s| s.x > 1000.0 || s.y > 1000.0);
            if already_reprojected {
                i.clone()
            } else {
                reproject_poly(
                    &s.get_inner(),
                    &source,
                    &target,
                    UseRadians::ForSourceAndTarget,
                    true,
                )
            }
        });

        proj4rs::transform::transform(&source, &target, coords.as_mut_slice()).ok()?;
        let points = coords
            .iter()
            .map(|p| SvgPoint { x: p.0, y: p.1 })
            .collect::<Vec<_>>();

        let mut max_x = points.get(0)?.x;
        let mut min_x = points.get(0)?.x;
        let mut max_y = points.get(0)?.y;
        let mut min_y = points.get(0)?.y;

        for p in points {
            if p.x > max_x {
                max_x = p.x;
            }
            if p.x < min_x {
                min_x = p.x;
            }
            if p.y > max_y {
                max_y = p.y;
            }
            if p.y < min_y {
                min_y = p.y;
            }
        }

        Some(RissExtentReprojected {
            crs: target_crs.to_string(),
            scale: self.scale,
            max_x,
            min_x,
            max_y,
            min_y,
            rissgebiet,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RissExtentReprojected {
    pub crs: String,
    pub scale: f64,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub rissgebiet: Option<SvgPolygonInner>,
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
    pub fn get_rect_line_poly(&self) -> SvgPolygonInner {
        let mut s = SvgPolygonInner {
            outer_ring: self.get_rect_line(),
            inner_rings: Vec::new(),
        };
        s.correct_winding_order();
        s
    }

    pub fn get_rect_line(&self) -> SvgLine {
        let rect = self.get_rect();
        SvgLine {
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
            ],
        }
    }
    pub fn get_poly(&self) -> SvgPolygonInner {
        let header_width_mm = 175.0;
        let header_height_mm = 35.0;
        let header_width_m = header_width_mm * self.scale / 1000.0;
        let header_height_m = header_height_mm * self.scale / 1000.0;

        //
        //           4------5
        //    header |      |
        //  2--------3      |
        //  |               |
        //  |               |
        //  |               |
        //  |               |
        //  |               |
        //  1---------------6
        //

        SvgPolygonInner {
            outer_ring: SvgLine {
                points: vec![
                    SvgPoint {
                        x: self.min_x,
                        y: self.min_y,
                    }, // 1
                    SvgPoint {
                        x: self.min_x,
                        y: self.max_y - header_height_m,
                    }, // 2
                    SvgPoint {
                        x: self.min_x + header_width_m,
                        y: self.max_y - header_height_m,
                    }, // 3
                    SvgPoint {
                        x: self.min_x + header_width_m,
                        y: self.max_y,
                    }, // 4
                    SvgPoint {
                        x: self.max_x,
                        y: self.max_y,
                    }, // 5
                    SvgPoint {
                        x: self.max_x,
                        y: self.min_y,
                    }, // 6
                    SvgPoint {
                        x: self.min_x,
                        y: self.min_y,
                    }, // 1
                ],
            },
            inner_rings: Vec::new(),
        }
        .round_to_3dec()
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
    #[serde(default)]
    pub rissgebiet: Option<SvgPolygon>,
}

#[derive(
    Debug, Default, Copy, Clone, Ord, Eq, Hash, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct RissConfigId([u64; 4]);

impl RissConfig {
    pub fn migrate_new(&self) -> Self {
        let rg = self.rissgebiet.as_ref().map(|s| s.migrate());
        Self {
            rissgebiet: rg,
            ..self.clone()
        }
    }
    pub fn migrate_old(&self, source_proj: &str) -> Self {
        let rg = self
            .rissgebiet
            .as_ref()
            .and_then(|s| Some(SvgPolygon::Old({
                let poly_needs_reprojection = s.get_inner().outer_ring.points.iter().any(|s| s.x > 1000.0 || s.y > 1000.0);
                if poly_needs_reprojection {
                    reproject_poly_back_into_latlon(&s.get_inner(), source_proj).ok()?
                } else {
                    s.get_inner()
                }
            })));

        let orig = SvgPoint { x: self.lon, y: self.lat };
        let latlon = if self.lat > 1000.0 {
            reproject_point_back_into_latlon(&orig, source_proj).ok().unwrap_or(orig)
        } else {
            orig
        };
        let lat = latlon.y;
        let lon = latlon.x;
        Self {
            rissgebiet: rg,
            lat,
            lon,
            ..self.clone()
        }
    }
    pub fn get_id(&self) -> RissConfigId {
        use highway::{
            HighwayHash,
            HighwayHasher,
        };
        let mut bytes = self.lat.to_le_bytes().to_vec();
        bytes.extend(self.lon.to_le_bytes().iter());
        RissConfigId(HighwayHasher::default().hash256(&bytes))
    }

    pub fn get_extent_special(&self, utm_crs: &str) -> Option<RissExtent> {
        self.get_extent(utm_crs, 16.5 * 2.0)
    }

    pub fn get_extent(&self, utm_crs: &str, padding_mm: f64) -> Option<RissExtent> {
        let height = self.height_mm as f64 - padding_mm;
        let width = self.width_mm as f64 - padding_mm;
        let total_map_meter_vert = height * (self.scale as f64 / 1000.0);
        let total_map_meter_horz = width * (self.scale as f64 / 1000.0);

        let utm_result = if self.lon > 1000.0 || self.lat > 1000.0 {
            SvgPoint {
                x: self.lon,
                y: self.lat,
            }
        } else {
                reproject_point_into_latlon(
                &SvgPoint {
                    x: self.lon,
                    y: self.lat,
                },
                utm_crs,
            )
            .ok()?
        };

        let north_utm = utm_result.y + (total_map_meter_vert / 2.0);
        let south_utm = utm_result.y - (total_map_meter_vert / 2.0);
        let east_utm = utm_result.x + (total_map_meter_horz / 2.0);
        let west_utm = utm_result.x - (total_map_meter_horz / 2.0);

        let north_east_deg = reproject_point_back_into_latlon(
            &SvgPoint {
                x: east_utm,
                y: north_utm,
            },
            utm_crs,
        )
        .ok()?;

        let south_west_deg = reproject_point_back_into_latlon(
            &SvgPoint {
                x: west_utm,
                y: south_utm,
            },
            utm_crs,
        )
        .ok()?;

        Some(RissExtent {
            coords: vec![
                LatLng {
                    lat: north_east_deg.y,
                    lng: north_east_deg.x,
                },
                LatLng {
                    lat: south_west_deg.y,
                    lng: south_west_deg.x,
                },
            ],
            scale: self.scale as f64,
            projection: utm_crs.to_string(),
            rissgebiet: self.rissgebiet.clone(),
        })
    }
}

pub struct Fluren {
    pub fluren: Vec<TaggedPolygon>,
}

pub struct FlurLabel {
    pub gemarkung_nr: usize,
    pub flur_nr: usize,
    pub pos: SvgPoint,
}

impl FlurLabel {
    pub fn text_pdf(&self, calc: &HeaderCalcConfig) -> Vec<String> {
        if calc.gemarkungs_nr == self.gemarkung_nr {
            vec![format!("Flur {}", self.flur_nr)]
        } else {
            vec![
                format!("Gem. {}", self.gemarkung_nr),
                format!("Flur {}", self.flur_nr),
            ]
        }
    }
    pub fn text(&self, calc: &HeaderCalcConfig) -> String {
        if calc.gemarkungs_nr == self.gemarkung_nr {
            format!("Flur {}", self.flur_nr)
        } else {
            format!("G{} Flur {}", self.gemarkung_nr, self.flur_nr)
        }
    }
}

impl Fluren {
    pub fn get_labels(&self, rect: &Option<SvgPolygonInner>) -> Vec<FlurLabel> {
        self.fluren
            .iter()
            .filter_map(|flst| {
                let poly = match rect {
                    Some(s) => intersect_polys(s, &flst.poly)
                        .get(0)
                        .unwrap_or_else(|| &flst.poly)
                        .clone(),
                    None => flst.poly.clone(),
                };
                let pos = poly.get_tertiary_label_pos()?;
                let gemarkung = flst
                    .attributes
                    .get("berechneteGemarkung")?
                    .parse::<usize>()
                    .ok()?;
                let flur = flst.attributes.get("AX_Flur")?.parse::<usize>().ok()?;
                Some(FlurLabel {
                    pos,
                    gemarkung_nr: gemarkung,
                    flur_nr: flur,
                })
            })
            .collect()
    }
}

pub struct FlurenInPdfSpace {
    pub fluren: Vec<TaggedPolygon>,
}

impl Fluren {
    pub fn to_pdf_space(&self, riss: &RissExtentReprojected, rc: &RissConfig) -> FlurenInPdfSpace {
        FlurenInPdfSpace {
            fluren: self
                .fluren
                .iter()
                .map(|tp| TaggedPolygon {
                    attributes: tp.attributes.clone(),
                    poly: poly_into_pdf_space(&tp.poly, riss, rc),
                })
                .collect(),
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
    pub fn get_labels(&self, rect: &Option<SvgPolygonInner>) -> Vec<TextPlacement> {
        self.flst
            .iter()
            .filter_map(|flst| {
                let poly = match rect {
                    Some(s) => intersect_polys(s, &flst.poly)
                        .get(0)
                        .unwrap_or_else(|| &flst.poly)
                        .clone(),
                    None => flst.poly.clone(),
                };
                let pos = poly.get_tertiary_label_pos()?;
                let flst_id =
                    FlstIdParsed::from_str(flst.attributes.get("flurstueckskennzeichen")?)
                        .parse_num()?
                        .format_dxf();
                Some(TextPlacement {
                    kuerzel: flst_id,
                    pos: pos,
                    ref_pos: pos,
                    poly,
                    status: TextStatus::StaysAsIs,
                    area: 1000,
                })
            })
            .collect()
    }

    pub fn to_pdf_space(
        &self,
        riss: &RissExtentReprojected,
        rc: &RissConfig,
    ) -> FlurstueckeInPdfSpace {
        FlurstueckeInPdfSpace {
            flst: self
                .flst
                .iter()
                .map(|tp| TaggedPolygon {
                    attributes: tp.attributes.clone(),
                    poly: poly_into_pdf_space(&tp.poly, riss, rc),
                })
                .collect(),
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
    pub fn to_pdf_space(
        &self,
        riss: &RissExtentReprojected,
        rc: &RissConfig,
    ) -> GebaeudeInPdfSpace {
        GebaeudeInPdfSpace {
            gebaeude: self
                .gebaeude
                .iter()
                .map(|tp| TaggedPolygon {
                    attributes: tp.attributes.clone(),
                    poly: poly_into_pdf_space(&tp.poly, riss, rc),
                })
                .collect(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PdfTargetUse {
    PreviewRiss,
    HintergrundCheck,
}

#[derive(Default)]
pub struct HintergrundCache {
    pub images: BTreeMap<RissConfigId, Vec<PdfImage>>,
}

impl HintergrundCache {
    pub async fn build(
        dop_source: Option<String>,
        dop_layers: Option<String>,
        risse: &[RissConfig],
        target_crs: &str,
    ) -> Self {
        let target_dpi = 96.0;
        let tile_size_px = 1024.0;

        let mut tiles = Vec::new();
        let len = risse.len();
        for (i, rc) in risse.iter().enumerate() {
            let id = rc.get_id();
            log_status(&format!("[{i} / {len}] BUILD id = {:?}", rc.get_id()));

            let ex = rc
                .get_extent(&target_crs, 0.0)
                .and_then(|q| q.reproject(target_crs));
            let riss_extent = match ex {
                Some(s) => s,
                None => continue,
            };

            let rect = riss_extent.get_rect();
            let target_px_width = rc.width_mm as f64 / 25.4 * target_dpi;
            let target_px_height = rc.height_mm as f64 / 25.4 * target_dpi;
            let num_tiles_x = (target_px_width / tile_size_px).ceil() as usize;
            let num_tiles_y = (target_px_height / tile_size_px).ceil() as usize;
            let tile_wh_mm = tile_size_px * 25.4 / target_dpi;
            let tile_wh_m = tile_wh_mm * rc.scale as f64 / 1000.0;

            for xi in 0..num_tiles_x {
                for yi in 0..num_tiles_y {
                    let t = (
                        id,
                        xi as f64 * tile_wh_mm,
                        yi as f64 * tile_wh_mm,
                        crate::uuid_wasm::FetchWmsImageRequest {
                            width_px: tile_size_px.round() as usize,
                            height_px: tile_size_px.round() as usize,
                            max_x: SvgPoint::round_f64(rect.min_x + ((xi + 1) as f64 * tile_wh_m)),
                            min_x: SvgPoint::round_f64(rect.min_x + (xi as f64 * tile_wh_m)),
                            max_y: SvgPoint::round_f64(rect.min_y + ((yi + 1) as f64 * tile_wh_m)),
                            min_y: SvgPoint::round_f64(rect.min_y + (yi as f64 * tile_wh_m)),
                            dop_layers: dop_layers.clone(),
                            dop_source: dop_source.clone(),
                        },
                    );
                    tiles.push(t);
                }
            }
        }

        web_sys::console::log_1(
            &format!("Fetche {} WMS Hintergrund Kacheln...", tiles.len()).into(),
        );

        let tiles_2 = tiles.iter().map(|s| s.3.clone()).collect::<Vec<_>>();
        let resolved_tiles = crate::uuid_wasm::get_wms_images(&tiles_2).await;

        let mut resolved = BTreeMap::new();
        for (resolved_data, (i, x, y, _)) in resolved_tiles.into_iter().zip(tiles.into_iter()) {
            if let Some(w) = resolved_data {
                resolved
                    .entry(i)
                    .or_insert_with(|| Vec::new())
                    .push(PdfImage {
                        x: Mm(x as f32),
                        y: Mm(y as f32),
                        dpi: target_dpi as f32,
                        image: w,
                    });
            }
        }

        log_status(&format!("OK: Hintergrund Cache fertig"));

        Self { images: resolved }
    }
}

pub struct PdfImage {
    pub x: Mm,
    pub y: Mm,
    pub dpi: f32,
    pub image: printpdf::Image,
}

const SCALE_OVERVIEW: f64 = 2400.0;

pub async fn export_overview(
    konfiguration: &Konfiguration,
    nas_xml: &NasXMLFile,
    split_nas: &SplitNasXml,
    csv: &CsvDataType,
    use_dgm: bool,
    use_background: bool,
) -> Vec<u8> {
    let calc = HeaderCalcConfig::from_csv(&split_nas, csv, &None);

    let split_nas = split_nas.only_retain_gemarkung(calc.gemarkungs_nr);

    let sf = split_nas.as_splitflaechen();

    let default_extent = match get_default_riss_extent(&sf, &[], &nas_xml.crs) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let default_extent = match default_extent.get_extent(&nas_xml.crs, 0.0) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let reprojected = match default_extent.reproject(&nas_xml.crs) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let width_mm = 297.0;
    let height_mm = 420.0;

    let width_m = width_mm * SCALE_OVERVIEW / 1000.0;
    let height_m = height_mm * SCALE_OVERVIEW / 1000.0;

    let mut riss_extente_reprojected = Vec::new();
    let mut max_y = reprojected.max_y;
    while max_y > reprojected.min_y {
        let mut min_x = reprojected.min_x;
        while min_x < reprojected.max_x {
            let extent = RissExtentReprojected {
                crs: nas_xml.crs.clone(),
                scale: SCALE_OVERVIEW,
                rissgebiet: None,
                min_x: min_x,
                max_x: min_x + width_m,
                min_y: max_y - height_m,
                max_y: max_y,
            };
            let utm_center = extent.get_rect().get_center();
            let latlon_center = crate::pdf::reproject_point_back_into_latlon(
                &SvgPoint {
                    x: utm_center.x,
                    y: utm_center.y,
                },
                &nas_xml.crs,
            )
            .unwrap_or_default();
            let rc = RissConfig {
                rissgebiet: None,
                crs: LATLON_STRING.to_string(),
                width_mm: width_mm as f32,
                height_mm: height_mm as f32,
                scale: SCALE_OVERVIEW as f32,
                lat: latlon_center.y,
                lon: latlon_center.x,
            };
            riss_extente_reprojected.push((rc, extent));
            min_x += width_m * 0.8;
        }
        max_y -= height_m * 0.8;
    }

    let mut files = Vec::new();

    let risse = riss_extente_reprojected
        .iter()
        .map(|s| s.0.clone())
        .collect::<Vec<_>>();
    let mut cache = if use_background {
        HintergrundCache::build(
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
            &risse,
            &nas_xml.crs,
        )
        .await
    } else {
        HintergrundCache::default()
    };

    let page_len = riss_extente_reprojected.len();

    for (i0, risse_contig) in riss_extente_reprojected.chunks(4).enumerate() {
        let (mut doc, page1, layer1) = PdfDocument::new(
            "Riss",
            Mm(width_mm as f32),
            Mm(height_mm as f32),
            &format!("Uebersicht"),
        );

        doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
            requires_icc_profile: false,
            requires_xmp_metadata: false,
            ..Default::default()
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

        let (page0_idx, layer0_idx) = (page1, layer1);
        let (page1_idx, layer1_idx) =
            doc.add_page(Mm(width_mm as f32), Mm(height_mm as f32), "Übersicht");
        let (page2_idx, layer2_idx) =
            doc.add_page(Mm(width_mm as f32), Mm(height_mm as f32), "Übersicht");
        let (page3_idx, layer3_idx) =
            doc.add_page(Mm(width_mm as f32), Mm(height_mm as f32), "Übersicht");

        for (i, (rc, extent)) in risse_contig.iter().enumerate() {
            let i_real = i0 * 4 + i;
            let (page_idx, layer_idx) = match i {
                0 => (page0_idx, layer0_idx),
                1 => (page3_idx, layer3_idx),
                2 => (page2_idx, layer2_idx),
                3 => (page1_idx, layer1_idx),
                _ => continue,
            };

            let mini_split_nas = get_mini_nas_xml(&split_nas, &extent);
            let flst = get_flurstuecke(nas_xml, &extent);
            let fluren = get_fluren(nas_xml, &Some(extent.get_rect()));
            let gebaeude = get_gebaeude(nas_xml, &extent);
            let riss_rect = extent.get_rect();
            let sf = sf
                .iter()
                .filter_map(|s| {
                    if s.poly_cut.get_rect().overlaps_rect(&riss_rect) {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let aenderungen_texte =
                crate::ui::AenderungenIntersections::get_texte(&sf, &extent.get_rect_line_poly());

            let beschriftungen = crate::optimize::optimize_labels(
                &mini_split_nas,
                &sf,
                &gebaeude,
                &[],
                &aenderungen_texte,
                &OptimizeConfig::new(&rc, &extent, 0.5 /* mm */),
            );

            let page = doc.get_page(page_idx);
            let mut layer = page.get_layer(layer_idx);
            let mut has_background = false;

            let nutzungsarten =
                reproject_splitnas_into_pdf_space(&mini_split_nas, &extent, &rc, &mut Vec::new());

            for i in cache.images.remove(&rc.get_id()).unwrap_or_default() {
                i.image.add_to_layer(
                    layer.clone(),
                    ImageTransform {
                        translate_x: i.x.into(),
                        translate_y: i.y.into(),
                        scale_x: Some(300.0 / i.dpi),
                        scale_y: Some(300.0 / i.dpi),
                        ..Default::default()
                    },
                );
                has_background = true;
            }

            let _ = write_nutzungsarten(&mut layer, &nutzungsarten, &konfiguration, has_background);
            let _ = write_gebaeude(
                &mut layer,
                &gebaeude.to_pdf_space(&extent, &rc),
                has_background,
            );
            let _ = write_flurstuecke(&mut layer, &flst.to_pdf_space(&extent, &rc), has_background);
            let _ = write_fluren(
                &mut layer,
                &fluren.to_pdf_space(&extent, &rc),
                &konfiguration,
                has_background,
            );
            let _ = write_flurstuecke_label(
                &mut layer,
                &helvetica,
                &flst,
                &rc,
                &extent,
                has_background,
            );
            let _ = write_flur_texte(
                &mut layer,
                &fluren,
                &helvetica,
                &rc,
                &extent,
                &calc,
                has_background,
            );

            let _ = write_splitflaechen_beschriftungen(
                &mut layer,
                &helvetica,
                &extent,
                &rc,
                &beschriftungen,
                has_background,
            );

            let _ = write_border(
                &mut layer,
                &rc,
                &ProjektInfo::default(),
                &calc,
                &times_roman,
                &times_roman_bold,
                None,
                PADDING / 6.0,
            );
            log_status(&format!("ok done page {i_real} / {page_len}"));
        }

        let bytes = doc.save_to_bytes().unwrap_or_default();
        files.push((None, PathBuf::from(format!("Uebersicht{i0}.pdf")), bytes));
    }

    log_status("ok done PDF");
    crate::zip::write_files_to_zip(files)
}

pub fn generate_pdf_internal(
    hintergrundbilder: Vec<PdfImage>,
    riss_von: (usize, usize), // Riss X von Y
    projekt_info: &ProjektInfo,
    calc: &HeaderCalcConfig,
    konfiguration: &Konfiguration,
    nutzungsarten: &SplitNasXml,
    rc: &RissConfig,
    riss_extent: &RissExtentReprojected,
    rote_linien: &Vec<SvgLine>,                // in ETRS space
    na_untergehend_linien: &Vec<SvgLine>,      // in ETRS space
    beschriftungen: &[OptimizedTextPlacement], // in ETRS space
    fluren: &Fluren,                           // in ETRS space,
    flst: &Flurstuecke,                        // in ETRS space
    gebaeude: &Gebaeude,                       // in ETRS space
) -> Vec<u8> {
    let (num_riss, total_risse) = riss_von;

    let (mut doc, page1, layer1) = PdfDocument::new(
        "Riss",
        Mm(rc.width_mm),
        Mm(rc.height_mm),
        &format!("Riss {} / {}", riss_von.0, riss_von.1),
    );

    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        ..Default::default()
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

    let mut has_background = false;
    for i in hintergrundbilder {
        i.image.add_to_layer(
            layer.clone(),
            ImageTransform {
                translate_x: i.x.into(),
                translate_y: i.y.into(),
                scale_x: Some(300.0 / i.dpi),
                scale_y: Some(300.0 / i.dpi),
                ..Default::default()
            },
        );
        has_background = true;
    }

    let nutzungsarten =
        reproject_splitnas_into_pdf_space(&nutzungsarten, &riss_extent, rc, &mut Vec::new());

    let _ = write_nutzungsarten(&mut layer, &nutzungsarten, &konfiguration, has_background);

    log_status(&format!("[{num_riss} / {total_risse}] Rendere Gebäude..."));
    let _ = write_gebaeude(
        &mut layer,
        &gebaeude.to_pdf_space(riss_extent, rc),
        has_background,
    );

    log_status(&format!(
        "[{num_riss} / {total_risse}] Rendere Flurstücke..."
    ));
    let _ = write_flurstuecke(
        &mut layer,
        &flst.to_pdf_space(riss_extent, rc),
        has_background,
    );

    log_status(&format!("[{num_riss} / {total_risse}] Rendere Fluren..."));
    let _ = write_fluren(
        &mut layer,
        &fluren.to_pdf_space(riss_extent, rc),
        &konfiguration,
        has_background,
    );

    log_status(&format!(
        "[{num_riss} / {total_risse}] Rendere Flurstücke Texte..."
    ));
    let _ = write_flurstuecke_label(
        &mut layer,
        &helvetica,
        &flst,
        rc,
        riss_extent,
        has_background,
    );

    log_status(&format!(
        "[{num_riss} / {total_risse}] Rendere Fluren Texte..."
    ));
    let _ = write_flur_texte(
        &mut layer,
        &fluren,
        &helvetica,
        rc,
        &riss_extent,
        calc,
        has_background,
    );

    log_status(&format!(
        "[{num_riss} / {total_risse}] Rendere rote Linien..."
    ));
    let rote_linien = rote_linien
        .iter()
        .map(|l| line_into_pdf_space(&l, riss_extent, rc))
        .collect::<Vec<_>>();
    let _ = write_rote_linien(&mut layer, &rote_linien);

    log_status(&format!(
        "[{num_riss} / {total_risse}] Rendere NA untergehend Linien... {} Linien",
        na_untergehend_linien.len()
    ));
    let na_untergehend_linien = crate::geograf::lines_to_points(&na_untergehend_linien)
        .iter()
        .map(|(p, a)| (point_into_pdf_space(p, riss_extent, rc), *a))
        .collect::<Vec<_>>();
    let _ = write_na_untergehend_linien(&mut layer, &na_untergehend_linien);

    log_status(&format!(
        "[{num_riss} / {total_risse}] Rendere Beschriftungen..."
    ));
    let _ = write_splitflaechen_beschriftungen(
        &mut layer,
        &helvetica,
        riss_extent,
        rc,
        &beschriftungen,
        has_background,
    );

    let _ = write_border(
        &mut layer,
        &rc,
        projekt_info,
        calc,
        &times_roman,
        &times_roman_bold,
        Some(riss_von),
        16.5,
    );

    log_status(&format!("[{num_riss} / {total_risse}] PDF fertig."));

    doc.save_to_bytes().unwrap_or_default()
}

pub fn reproject_rissgebiete_into_target_space(
    risse: &Risse,
    target_proj: &str,
) -> Risse {

    risse.iter().filter_map(|(k, v)| {

        let utm_result = if v.lon > 1000.0 || v.lat > 1000.0 {
            SvgPoint {
                x: v.lon,
                y: v.lat,
            }
        } else {
                reproject_point_into_latlon(
                &SvgPoint {
                    x: v.lon,
                    y: v.lat,
                },
                target_proj,
            )
            .ok()?
        };

        let rissgebiet = v.rissgebiet.as_ref().and_then(|s| {

            let source = proj4rs::Proj::from_proj_string(LATLON_STRING).ok()?;
            let target = proj4rs::Proj::from_proj_string(target_proj).ok()?;

            let i = s.get_inner();
            let already_reprojected = i.outer_ring.points.iter().any(|s| s.x > 1000.0 || s.y > 1000.0);
            if already_reprojected {
                Some(i.clone())
            } else {
                Some(reproject_poly(
                    &s.get_inner(),
                    &source,
                    &target,
                    UseRadians::ForSourceAndTarget,
                    true,
                ))
            }
        }).map(|s| SvgPolygon::Old(s));

        Some((k.clone(), RissConfig {
            lat: utm_result.y,
            lon: utm_result.x,
            crs: target_proj.to_string(),
            width_mm: v.width_mm,
            height_mm: v.height_mm,
            scale: v.scale,
            rissgebiet,
        }))
    }).collect()
}

pub fn reproject_aenderungen_into_target_space(
    aenderungen: &Aenderungen,
    target_proj: &str,
) -> Result<Aenderungen, String> {

    let already_reprojected = aenderungen.na_polygone_neu.iter().any(|s| s.1.poly.get_inner().outer_ring.points.iter().any(|s| s.x > 1000.0 || s.y > 1000.0));
    
    if already_reprojected {
        return Ok(aenderungen.clone());
    }
    
    let target_proj = proj4rs::Proj::from_proj_string(&target_proj)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", target_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
        .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    Ok(Aenderungen {
        gebaeude_loeschen: aenderungen.gebaeude_loeschen.clone(),
        na_definiert: aenderungen.na_definiert.clone(),
        na_polygone_neu: aenderungen
            .na_polygone_neu
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    PolyNeu {
                        poly: SvgPolygon::Old(crate::nas::reproject_poly(
                            &v.poly.get_inner(),
                            &latlon_proj,
                            &target_proj,
                            UseRadians::ForSourceAndTarget,
                            true,
                        )),
                        nutzung: v.nutzung.clone(),
                        locked: v.locked,
                    },
                )
            })
            .collect(),
    })
}

pub fn reproject_point_into_latlon(p: &SvgPoint, target_proj: &str) -> Result<SvgPoint, String> {
    let target_proj = proj4rs::Proj::from_proj_string(&target_proj)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", target_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
        .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    let mut point3d = (p.x.to_radians(), p.y.to_radians(), 0.0_f64);
    proj4rs::transform::transform(&latlon_proj, &target_proj, &mut point3d)
        .map_err(|e| format!("error reprojecting: {e}"))?;

    Ok(SvgPoint {
        x: point3d.0,
        y: point3d.1,
    })
}

pub fn reproject_point_back_into_latlon(
    p: &SvgPoint,
    source_proj: &str,
) -> Result<SvgPoint, String> {
    let source_proj = proj4rs::Proj::from_proj_string(&source_proj)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", source_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
        .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    let mut point3d = (p.x, p.y, 0.0_f64);
    proj4rs::transform::transform(&source_proj, &latlon_proj, &mut point3d)
        .map_err(|e| format!("error reprojecting: {e}"))?;

    Ok(SvgPoint {
        x: point3d.0.to_degrees(),
        y: point3d.1.to_degrees(),
    })
}

pub fn reproject_poly_back_into_latlon(
    poly: &SvgPolygonInner,
    source_proj: &str,
) -> Result<SvgPolygonInner, String> {
    let source_proj = proj4rs::Proj::from_proj_string(&source_proj)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", source_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
        .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    Ok(crate::nas::reproject_poly(
        poly,
        &source_proj,
        &latlon_proj,
        UseRadians::None,
        false,
    ))
}

pub fn reproject_aenderungen_back_into_latlon(
    aenderungen: &Aenderungen,
    source_proj: &str,
) -> Result<Aenderungen, String> {
    let source_proj = proj4rs::Proj::from_proj_string(&source_proj)
        .map_err(|e| format!("source_proj_string: {e}: {:?}", source_proj))?;

    let latlon_proj = proj4rs::Proj::from_proj_string(LATLON_STRING)
        .map_err(|e| format!("latlon_proj_string: {e}: {LATLON_STRING:?}"))?;

    Ok(Aenderungen {
        gebaeude_loeschen: aenderungen.gebaeude_loeschen.clone(),
        na_definiert: aenderungen.na_definiert.clone(),
        na_polygone_neu: aenderungen
            .na_polygone_neu
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    PolyNeu {
                        poly: SvgPolygon::Old(crate::nas::reproject_poly(
                            &v.poly.get_inner(),
                            &source_proj,
                            &latlon_proj,
                            UseRadians::None,
                            false,
                        )),
                        nutzung: v.nutzung.clone(),
                        locked: v.locked,
                    },
                )
            })
            .collect(),
    })
}

pub fn reproject_splitflaechen_into_pdf_space(
    splitflaechen: &[AenderungenIntersection],
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    _log: &mut Vec<String>,
) -> Result<Vec<AenderungenIntersection>, String> {
    let _target_riss = riss.get_rect();
    Ok(splitflaechen
        .iter()
        .map(|s| AenderungenIntersection {
            alt: s.alt.clone(),
            neu: s.neu.clone(),
            flst_id: s.flst_id.clone(),
            flst_id_part: s.flst_id_part.clone(),
            poly_cut: poly_into_pdf_space(&s.poly_cut, &riss, riss_config),
        })
        .collect())
}

#[inline(always)]
fn reproject_splitnas_into_pdf_space(
    split_flurstuecke: &SplitNasXml,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
    _log: &mut Vec<String>,
) -> SplitNasXml {
    let target_riss = quadtree_f32::Rect {
        min_x: riss.min_x,
        min_y: riss.min_y,
        max_x: riss.max_x,
        max_y: riss.max_y,
    };
    SplitNasXml {
        crs: "pdf".to_string(),
        flurstuecke_nutzungen: split_flurstuecke
            .flurstuecke_nutzungen
            .iter()
            .filter_map(|(k, v)| {
                let v = v
                    .iter()
                    .filter_map(|s| {
                        if s.get_rect().overlaps_rect(&target_riss) {
                            Some(TaggedPolygon {
                                attributes: s.attributes.clone(),
                                poly: poly_into_pdf_space(&s.poly, &riss, riss_config),
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if v.is_empty() {
                    None
                } else {
                    Some((k.clone(), v))
                }
            })
            .collect(),
    }
}

fn poly_into_pdf_space(
    poly: &SvgPolygonInner,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
) -> SvgPolygonInner {
    SvgPolygonInner {
        outer_ring: line_into_pdf_space(&poly.outer_ring, riss, riss_config),
        inner_rings: poly
            .inner_rings
            .iter()
            .map(|l| line_into_pdf_space(l, riss, riss_config))
            .collect(),
    }
}

fn line_into_pdf_space(
    line: &SvgLine,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
) -> SvgLine {
    SvgLine {
        points: line
            .points
            .iter()
            .map(|p| point_into_pdf_space(p, riss, riss_config))
            .collect(),
    }
}

fn point_into_pdf_space(
    p: &SvgPoint,
    riss: &RissExtentReprojected,
    riss_config: &RissConfig,
) -> SvgPoint {
    SvgPoint {
        x: (p.x - riss.min_x) / riss.width_m() * riss_config.width_mm as f64,
        y: (p.y - riss.min_y) / riss.height_m() * riss_config.height_mm as f64,
    }
}

fn write_na_untergehend_linien(
    layer: &mut PdfLayerReference,
    linien: &[(SvgPoint, AngleDegrees)],
) -> Option<()> {
    layer.save_graphics_state();

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 235.0,
        g: 140.0,
        b: 52.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(0.5);
    layer.set_line_cap_style(printpdf::LineCapStyle::Round);
    layer.set_line_join_style(printpdf::LineJoinStyle::Round);

    for (p, _a) in linien.iter() {
        let circle = calculate_points_for_circle(Mm(1.0), Mm(p.x as f32), Mm(p.y as f32));
        layer.add_line(printpdf::Line {
            points: circle,
            is_closed: true,
        });
        /*
        layer.add_line(printpdf::Line {
            points: l.points.iter().map(|p| (printpdf::Point {
                x: Mm(p.x as f32).into_pt(),
                y: Mm(p.y as f32).into_pt(),
            }, false)).collect(),
            is_closed: l.is_closed()
        })
         */
    }

    layer.restore_graphics_state();

    Some(())
}

fn write_flur_texte(
    layer: &mut PdfLayerReference,
    fluren: &Fluren,
    font: &IndirectFontRef,
    riss: &RissConfig,
    riss_extent: &RissExtentReprojected,
    calc: &HeaderCalcConfig,
    _has_background: bool,
) -> Option<()> {
    let flurcolor = csscolorparser::parse("#ee22ff")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let outline_color = csscolorparser::parse("#ffffff")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let texte = fluren
        .get_labels(&Some(riss_extent.get_poly()))
        .into_iter()
        .map(|fl| {
            (
                point_into_pdf_space(&fl.pos, riss_extent, riss),
                fl.text_pdf(calc),
            )
        })
        .collect::<Vec<_>>();

    layer.save_graphics_state();

    let fontsize = 20.0;
    layer.set_fill_color(flurcolor.clone());
    layer.set_outline_color(outline_color.clone());
    layer.set_outline_thickness(1.2);

    for (pos, t) in texte {
        layer.begin_text_section();
        layer.set_font(&font, fontsize);
        layer.set_line_height(fontsize);
        layer.set_text_rendering_mode(TextRenderingMode::FillStroke);
        layer.set_text_cursor(Mm(pos.x as f32), Mm(pos.y as f32));
        for v in t.iter() {
            layer.write_text(v, &font);
            layer.add_line_break();
        }
        layer.end_text_section();

        layer.begin_text_section();
        layer.set_font(&font, fontsize);
        layer.set_line_height(fontsize);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(pos.x as f32), Mm(pos.y as f32));
        for v in t.iter() {
            layer.write_text(v, &font);
            layer.add_line_break();
        }
        layer.end_text_section();
    }

    layer.restore_graphics_state();

    Some(())
}

fn write_rote_linien(layer: &mut PdfLayerReference, linien: &[SvgLine]) -> Option<()> {
    layer.save_graphics_state();

    layer.set_outline_color(printpdf::Color::Rgb(Rgb {
        r: 255.0,
        g: 0.0,
        b: 0.0,
        icc_profile: None,
    }));

    layer.set_outline_thickness(1.0);
    layer.set_line_cap_style(printpdf::LineCapStyle::Round);
    layer.set_line_join_style(printpdf::LineJoinStyle::Round);

    for l in linien.iter() {
        layer.add_line(printpdf::Line {
            points: l
                .points
                .iter()
                .map(|p| {
                    (
                        printpdf::Point {
                            x: Mm(p.x as f32).into_pt(),
                            y: Mm(p.y as f32).into_pt(),
                        },
                        false,
                    )
                })
                .collect(),
            is_closed: l.is_closed(),
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
    has_background: bool,
) -> Option<()> {
    let linien = beschriftungen
        .iter()
        .filter_map(|l| {
            let (start, end) = l.get_line()?;
            let start = point_into_pdf_space(&start, riss_extent, riss);
            let end = point_into_pdf_space(&end, riss_extent, riss);

            Some((
                l.optimized.status.clone(),
                printpdf::Line {
                    points: vec![
                        (
                            printpdf::Point {
                                x: Mm(start.x as f32).into_pt(),
                                y: Mm(start.y as f32).into_pt(),
                            },
                            false,
                        ),
                        (
                            printpdf::Point {
                                x: Mm(end.x as f32).into_pt(),
                                y: Mm(end.y as f32).into_pt(),
                            },
                            false,
                        ),
                    ],
                    is_closed: false,
                },
            ))
        })
        .collect::<Vec<_>>();

    let texte_alt = beschriftungen
        .iter()
        .filter(|s| s.optimized.status == TextStatus::Old)
        .map(|p| TextPlacement {
            kuerzel: p.optimized.kuerzel.clone(),
            status: p.optimized.status.clone(),
            pos: point_into_pdf_space(&p.optimized.pos, riss_extent, riss),
            ref_pos: point_into_pdf_space(&p.optimized.ref_pos, riss_extent, riss),
            area: p.optimized.area,
            poly: p.optimized.poly.clone(),
        })
        .collect::<Vec<_>>();

    let texte_neu = beschriftungen
        .into_iter()
        .filter(|s| s.optimized.status == TextStatus::New)
        .map(|p| TextPlacement {
            kuerzel: p.optimized.kuerzel.clone(),
            status: p.optimized.status.clone(),
            pos: point_into_pdf_space(&p.optimized.pos, riss_extent, riss),
            ref_pos: point_into_pdf_space(&p.optimized.ref_pos, riss_extent, riss),
            area: p.optimized.area,
            poly: p.optimized.poly.clone(),
        })
        .collect::<Vec<_>>();

    let texte_bleibt = beschriftungen
        .into_iter()
        .filter(|s| s.optimized.status == TextStatus::StaysAsIs)
        .map(|p| TextPlacement {
            kuerzel: p.optimized.kuerzel.clone(),
            status: p.optimized.status.clone(),
            pos: point_into_pdf_space(&p.optimized.pos, riss_extent, riss),
            ref_pos: point_into_pdf_space(&p.optimized.ref_pos, riss_extent, riss),
            area: p.optimized.area,
            poly: p.optimized.poly.clone(),
        })
        .collect::<Vec<_>>();

    let white = csscolorparser::parse("#ffffff")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let alt_color = csscolorparser::parse("#cc0000")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let neu_color = csscolorparser::parse("#00aa00")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let bleibt_color = csscolorparser::parse(if has_background { "#ff0000" } else { "#6082B6" })
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.save_graphics_state();

    let write_text = |t: &str, x: f64, y: f64, color: printpdf::Color| {
        layer.begin_text_section();
        layer.set_outline_color(white.clone());
        layer.set_outline_thickness(1.2);
        layer.set_fill_color(color.clone());
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::FillStroke);
        layer.set_text_cursor(Mm(x as f32), Mm(y as f32));
        layer.write_text(t, &font);
        layer.end_text_section();

        layer.begin_text_section();
        layer.set_fill_color(color.clone());
        layer.set_font(&font, 6.0);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(x as f32), Mm(y as f32));
        layer.write_text(t, &font);
        layer.end_text_section();
    };

    for t in texte_bleibt {
        write_text(&t.kuerzel, t.pos.x, t.pos.y, bleibt_color.clone());
    }

    for t in texte_alt {
        write_text(&t.kuerzel, t.pos.x, t.pos.y, alt_color.clone());
    }

    for t in texte_neu {
        write_text(&t.kuerzel, t.pos.x, t.pos.y, neu_color.clone());
    }

    layer.restore_graphics_state();

    layer.save_graphics_state();

    for (ts, li) in linien.iter() {
        let col = match ts {
            TextStatus::New => neu_color.clone(),
            TextStatus::StaysAsIs => bleibt_color.clone(),
            TextStatus::Old => alt_color.clone(),
        };
        layer.set_outline_color(white.clone());
        layer.set_outline_thickness(1.5);
        layer.add_line(li.clone());

        layer.set_outline_color(col.clone());
        layer.set_outline_thickness(1.0);
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
    num_riss_total_riss: Option<(usize, usize)>,
    border_width_mm: f32,
) -> Option<()> {
    use printpdf::Point;

    let add_rect = |x, y, w, h, paintmode| {
        let points = vec![
            (
                Point {
                    x: Mm(x).into(),
                    y: Mm(y).into(),
                },
                false,
            ),
            (
                Point {
                    x: Mm(x + w).into(),
                    y: Mm(y).into(),
                },
                false,
            ),
            (
                Point {
                    x: Mm(x + w).into(),
                    y: Mm(y + h).into(),
                },
                false,
            ),
            (
                Point {
                    x: Mm(x).into(),
                    y: Mm(y + h).into(),
                },
                false,
            ),
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
    add_rect(
        0.0,
        riss.height_mm - border_width_mm,
        riss.width_mm,
        border_width_mm,
        PaintMode::Fill,
    );
    add_rect(
        riss.width_mm - border_width_mm,
        0.0,
        border_width_mm,
        riss.height_mm,
        PaintMode::Fill,
    );

    add_rect(
        border_width_mm,
        border_width_mm,
        riss.width_mm - (border_width_mm * 2.0),
        riss.height_mm - (border_width_mm * 2.0),
        PaintMode::Stroke,
    );

    if let Some((num_riss, total_riss)) = num_riss_total_riss {
        add_rect(
            border_width_mm,
            riss.height_mm - border_width_mm - 35.0,
            175.0,
            35.0,
            PaintMode::Fill,
        );

        let _ = write_header(
            layer,
            info,
            calc,
            times_roman,
            times_roman_bold,
            num_riss,
            total_riss,
            riss.height_mm - border_width_mm - 35.0,
            border_width_mm,
        );
    }

    layer.restore_graphics_state();
    Some(())
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

    layer1.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(
        0.0, 0.0, 0.0, None,
    )));

    let text = format!("Ergänzungsriss: Tatsächliche Nutzung ( {num_riss} / {total_risse} )");
    layer1.use_text(
        &text,
        header_font_size,
        Mm(offset_right + 2.0),
        Mm(offset_top + 30.0),
        &times_roman_bold,
    );

    let text = "Gemeinde:";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 2.0),
        Mm(offset_top + 25.0),
        &times_roman,
    );

    let text = "Gemarkung:";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 2.0),
        Mm(offset_top + 17.0),
        &times_roman,
    );

    let text = "Flur";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 2.0),
        Mm(offset_top + 10.0),
        &times_roman,
    );

    let text = "Instrument/Nr.";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 2.0),
        Mm(offset_top + 3.0),
        &times_roman,
    );

    let text = "-";
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 32.0),
        Mm(offset_top + 2.0),
        &times_roman,
    );

    let text = "Flurstücke";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 20.0),
        Mm(offset_top + 10.0),
        &times_roman,
    );

    let text = "Bearbeitung beendet am:";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 62.0),
        Mm(offset_top + 25.0),
        &times_roman,
    );

    let text = format!(
        "Erstellt durch: {} ({})",
        info.erstellt_durch, info.beruf_kuerzel
    );
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 62.0),
        Mm(offset_top + 17.0),
        &times_roman,
    );

    let text = "Vermessungsstelle";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 62.0),
        Mm(offset_top + 10.0),
        &times_roman,
    );

    let text = "Grenztermin vom";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 104.0),
        Mm(offset_top + 25.0),
        &times_roman,
    );

    let text = "-";
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 120.0),
        Mm(offset_top + 21.0),
        &times_roman,
    );

    let text = "Verwendete Vermessungsun-";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 104.0),
        Mm(offset_top + 17.0),
        &times_roman,
    );
    let text = "terlagen";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 104.0),
        Mm(offset_top + 14.0),
        &times_roman,
    );

    let text = format!("ALKIS ({})", info.alkis_aktualitaet);
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 104.0),
        Mm(offset_top + 10.0),
        &times_roman,
    );
    let text = format!("Orthophoto ({})", info.orthofoto_datum);
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 104.0),
        Mm(offset_top + 6.5),
        &times_roman,
    );
    let text = format!("GIS-Feldblöcke ({})", info.gis_feldbloecke_datum);
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 104.0),
        Mm(offset_top + 3.0),
        &times_roman,
    );

    let text = "Archivblatt: *";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 140.0),
        Mm(offset_top + 21.0),
        &times_roman,
    );

    let text = "Antrags-Nr.: *";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 140.0),
        Mm(offset_top + 17.0),
        &times_roman,
    );

    let text = info.antragsnr.trim().to_string();
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 140.0),
        Mm(offset_top + 14.0),
        &times_roman,
    );

    let text = "Katasteramt:";
    layer1.use_text(
        text,
        small_font_size,
        Mm(offset_right + 140.0),
        Mm(offset_top + 10.0),
        &times_roman,
    );

    let text = info.katasteramt.trim();
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 150.0),
        Mm(offset_top + 2.0),
        &times_roman,
    );

    let text = info.gemeinde.trim();
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 20.0),
        Mm(offset_top + 21.0),
        &times_roman,
    );

    let text = format!("{} ({})", info.gemarkung.trim(), calc.gemarkungs_nr);
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 20.0),
        Mm(offset_top + 14.0),
        &times_roman,
    );

    let text = calc.get_flst_string();
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 32.0),
        Mm(offset_top + 7.0),
        &times_roman,
    );

    let text = info.bearbeitung_beendet_am.trim();
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 73.0),
        Mm(offset_top + 21.0),
        &times_roman,
    );

    let text = info.vermessungsstelle.trim();
    layer1.use_text(
        text,
        medium_font_size,
        Mm(offset_right + 68.0),
        Mm(offset_top + 2.0),
        &times_roman,
    );

    let text = calc.get_fluren_string();
    let fluren_len = calc.get_fluren_len();
    let offset_right_fluren = match fluren_len {
        0 => offset_right + 8.0,
        1 => offset_right + 8.0,
        2 => offset_right + 6.0,
        3 => offset_right + 4.0,
        4 => offset_right + 2.0,
        _ => offset_right + 4.0,
    };
    layer1.use_text(
        &text,
        medium_font_size,
        Mm(offset_right_fluren),
        Mm(offset_top + 7.0),
        &times_roman,
    );

    let lines = &[
        (
            (offset_right + 0.0, offset_top + 28.0),
            (offset_right + 139.0, offset_top + 28.0),
        ),
        (
            (offset_right + 0.0, offset_top + 20.0),
            (offset_right + 175.0, offset_top + 20.0),
        ),
        (
            (offset_right + 0.0, offset_top + 13.0),
            (offset_right + 102.0, offset_top + 13.0),
        ),
        (
            (offset_right + 139.0, offset_top + 13.0),
            (offset_right + 175.0, offset_top + 13.0),
        ),
        (
            (offset_right + 0.0, offset_top + 6.0),
            (offset_right + 60.0, offset_top + 6.0),
        ),
        (
            (offset_right + 17.0, offset_top + 13.0),
            (offset_right + 17.0, offset_top + 6.0),
        ),
        (
            (offset_right + 60.0, offset_top + 28.0),
            (offset_right + 60.0, offset_top + 0.0),
        ),
        (
            (offset_right + 102.0, offset_top + 28.0),
            (offset_right + 102.0, offset_top + 0.0),
        ),
        (
            (offset_right + 139.0, offset_top + 28.0),
            (offset_right + 139.0, offset_top + 0.0),
        ),
    ];

    layer1.set_outline_thickness(0.5);
    layer1.set_outline_color(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    for ((x0, y0), (x1, y1)) in lines.iter() {
        layer1.add_line(printpdf::Line {
            points: vec![
                (
                    printpdf::Point {
                        x: Mm(*x0).into(),
                        y: Mm(*y0).into(),
                    },
                    false,
                ),
                (
                    printpdf::Point {
                        x: Mm(*x1).into(),
                        y: Mm(*y1).into(),
                    },
                    false,
                ),
            ],
            is_closed: false,
        })
    }

    layer1.restore_graphics_state();

    Some(())
}

fn write_nutzungsarten(
    layer: &mut PdfLayerReference,
    split_flurstuecke: &SplitNasXml,
    style: &Konfiguration,
    has_background: bool,
) -> Option<()> {
    let flurstueck_nutzungen_grouped_by_ebene =
        if style.pdf.nutzungsarten.is_empty() || has_background {
            split_flurstuecke
                .flurstuecke_nutzungen
                .iter()
                .map(|(f, v)| {
                    (
                        PdfEbenenStyle::default_grau(f, has_background),
                        v.iter().collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            let mut fl_btree = BTreeMap::new();
            for (_flst_id, flst_parts) in split_flurstuecke.flurstuecke_nutzungen.iter() {
                for f in flst_parts.iter() {
                    let flst_kuerzel_alt = match f.get_auto_kuerzel() {
                        Some(s) => s,
                        None => continue,
                    };

                    let flst_style = style.pdf.nutzungsarten.iter().find_map(|(k, v)| {
                        if v.kuerzel != flst_kuerzel_alt {
                            None
                        } else {
                            Some((k.clone(), v.clone()))
                        }
                    });

                    let (flst_style_id, _flst_style) = match flst_style {
                        Some(s) => s,
                        None => continue,
                    };

                    fl_btree
                        .entry(flst_style_id)
                        .or_insert_with(|| Vec::new())
                        .push(f);
                }
            }

            style
                .pdf
                .layer_ordnung
                .iter()
                .filter_map(|s| {
                    let polys = fl_btree.get(s)?.clone();
                    let style = style.pdf.nutzungsarten.get(s)?.clone();
                    Some((style, polys))
                })
                .collect::<Vec<_>>()
        };

    // log.push(serde_json::to_string(&flurstueck_nutzungen_grouped_by_ebene).unwrap_or_default());

    let white = Some("#ffffff")
        .and_then(|s| csscolorparser::parse(&s).ok())
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or_else(|| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                icc_profile: None,
            })
        });

    for (style, polys) in flurstueck_nutzungen_grouped_by_ebene.iter() {
        layer.save_graphics_state();

        let mut paintmode = PaintMode::Fill;
        let fill_color = style
            .fill_color
            .as_ref()
            .and_then(|s| csscolorparser::parse(&s).ok())
            .map(|c| {
                printpdf::Color::Rgb(printpdf::Rgb {
                    r: c.r as f32,
                    g: c.g as f32,
                    b: c.b as f32,
                    icc_profile: None,
                })
            });

        let outline_color: Option<printpdf::Color> = style
            .outline_color
            .as_ref()
            .and_then(|s| csscolorparser::parse(&s).ok())
            .map(|c| {
                printpdf::Color::Rgb(printpdf::Rgb {
                    r: c.r as f32,
                    g: c.g as f32,
                    b: c.b as f32,
                    icc_profile: None,
                })
            });

        // paint white outline

        layer.save_graphics_state();
        layer.set_outline_color(white.clone());
        layer.set_outline_thickness(1.5);
        for poly in polys.iter() {
            layer.add_polygon(translate_poly(&poly.poly, PaintMode::Stroke));
        }
        layer.restore_graphics_state();

        // let outline_thickness = style.outline_thickness.unwrap_or(1.0);
        layer.set_outline_thickness(1.0);

        if let Some(fc) = fill_color.as_ref() {
            layer.set_fill_color(fc.clone());
        }

        if let Some(oc) = outline_color.as_ref() {
            layer.set_outline_color(oc.clone());
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

pub fn get_mini_nas_xml(xml: &SplitNasXml, riss: &RissExtentReprojected) -> SplitNasXml {
    let rb = riss.get_rect();

    SplitNasXml {
        crs: xml.crs.clone(),
        flurstuecke_nutzungen: xml
            .flurstuecke_nutzungen
            .iter()
            .map(|(k, v)| {
                let mut v = v.clone();
                v.retain(|s| rb.overlaps_rect(&s.get_rect()));
                (k.clone(), v)
            })
            .collect(),
    }
}

pub fn get_flurstuecke(xml: &NasXMLFile, riss: &RissExtentReprojected) -> Flurstuecke {
    let mut flst = xml.ebenen.get("AX_Flurstueck").cloned().unwrap_or_default();
    flst.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    Flurstuecke { flst }
}

pub fn get_gebaeude(xml: &NasXMLFile, riss: &RissExtentReprojected) -> Gebaeude {
    let mut gebaeude = xml.ebenen.get("AX_Gebaeude").cloned().unwrap_or_default();
    gebaeude.retain(|s| {
        let rb = riss.get_rect();
        rb.overlaps_rect(&s.get_rect())
    });

    Gebaeude { gebaeude }
}

pub fn get_fluren(xml: &NasXMLFile, rect: &Option<quadtree_f32::Rect>) -> Fluren {
    let mut flst = xml.ebenen.get("AX_Flurstueck").cloned().unwrap_or_default();
    if let Some(q) = rect.as_ref() {
        flst.retain(|s| q.overlaps_rect(&s.get_rect()));
    }

    let mut fluren_map = BTreeMap::new();
    for v in flst.iter() {
        let flst = v
            .attributes
            .get("flurstueckskennzeichen")
            .and_then(|s| FlstIdParsed::from_str(s).parse_num());

        let flst = match flst {
            Some(s) => s,
            None => continue,
        };

        fluren_map
            .entry(flst.gemarkung)
            .or_insert_with(|| BTreeMap::new())
            .entry(flst.flur)
            .or_insert_with(|| Vec::new())
            .push(v);
    }

    Fluren {
        fluren: fluren_map
            .iter()
            .flat_map(|(gemarkung_nr, fluren)| {
                fluren.iter().flat_map(|(flur_nr, s)| {
                    let polys = s.iter().map(|s| s.poly.clone()).collect::<Vec<_>>();
                    crate::ops::join_polys(&polys)
                    .into_iter().map(|mut joined| {
                        joined.inner_rings = Vec::new();
                        TaggedPolygon {
                            attributes: vec![
                                ("berechneteGemarkung".to_string(), gemarkung_nr.to_string()),
                                ("AX_Flur".to_string(), flur_nr.to_string()),
                            ]
                            .into_iter()
                            .collect(),
                            poly: joined,
                        }
                    })
                })
            })
            .collect(),
    }
}

fn write_flurstuecke_label(
    layer: &mut PdfLayerReference,
    font: &IndirectFontRef,
    flst: &Flurstuecke,
    riss_config: &RissConfig,
    riss: &RissExtentReprojected,
    has_background: bool,
) -> Option<()> {
    layer.save_graphics_state();

    let fill_color = csscolorparser::parse(if has_background { "#ffffff" } else { "#000000" })
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let outline_color = csscolorparser::parse(if has_background { "#000000" } else { "#ffffff" })
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let fontsize = 5.0;
    layer.set_fill_color(fill_color.clone());
    layer.set_outline_color(outline_color.clone());
    layer.set_outline_thickness(1.0);

    for tp in flst.flst.iter() {
        let flst = match tp
            .attributes
            .get("flurstueckskennzeichen")
            .and_then(|s| FlstIdParsed::from_str(s).parse_num())
        {
            Some(s) => s,
            None => continue,
        };
        let pos = match tp
            .poly
            .get_secondary_label_pos()
            .or(tp.poly.get_label_pos())
        {
            Some(s) => point_into_pdf_space(&s, riss, riss_config),
            None => continue,
        };
        layer.begin_text_section();
        layer.set_font(&font, fontsize);
        layer.set_line_height(fontsize);
        layer.set_text_rendering_mode(TextRenderingMode::FillStroke);
        layer.set_text_cursor(Mm(pos.x as f32), Mm(pos.y as f32));
        layer.write_text(flst.format_str(), &font);
        layer.end_text_section();

        layer.begin_text_section();
        layer.set_font(&font, fontsize);
        layer.set_line_height(fontsize);
        layer.set_text_rendering_mode(TextRenderingMode::Fill);
        layer.set_text_cursor(Mm(pos.x as f32), Mm(pos.y as f32));
        layer.write_text(flst.format_str(), &font);
        layer.end_text_section();
    }

    layer.restore_graphics_state();

    Some(())
}

fn write_flurstuecke(
    layer: &mut PdfLayerReference,
    flst: &FlurstueckeInPdfSpace,
    has_background: bool,
) -> Option<()> {
    layer.save_graphics_state();

    let outline_color = csscolorparser::parse(if has_background { "#ffffff" } else { "#00000" })
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.set_outline_color(outline_color);

    layer.set_outline_thickness(1.0);

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
    has_background: bool,
) -> Option<()> {
    let fill_color = csscolorparser::parse("#808080")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    let outline_color = csscolorparser::parse(if has_background { "#ffffff" } else { "#000000" })
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.save_graphics_state();

    layer.set_fill_color(fill_color);

    layer.set_outline_color(outline_color);

    layer.set_outline_thickness(0.5);

    for tp in gebaeude.gebaeude.iter() {
        let poly = translate_poly(
            &tp.poly,
            if has_background {
                PaintMode::Stroke
            } else {
                PaintMode::FillStroke
            },
        );
        layer.add_polygon(poly);
    }

    layer.restore_graphics_state();

    Some(())
}

fn write_fluren(
    layer: &mut PdfLayerReference,
    fluren: &FlurenInPdfSpace,
    _style: &Konfiguration,
    _has_background: bool,
) -> Option<()> {
    layer.save_graphics_state();

    let outline_color = csscolorparser::parse("#F8196F")
        .ok()
        .map(|c| {
            printpdf::Color::Rgb(printpdf::Rgb {
                r: c.r as f32,
                g: c.g as f32,
                b: c.b as f32,
                icc_profile: None,
            })
        })
        .unwrap_or(printpdf::Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    layer.set_overprint_stroke(true);
    layer.set_blend_mode(printpdf::BlendMode::Seperable(
        printpdf::SeperableBlendMode::Normal,
    ));

    layer.set_outline_color(outline_color);

    layer.set_outline_thickness(3.0);

    for tp in fluren.fluren.iter() {
        let poly = translate_poly(&tp.poly, PaintMode::Stroke);
        layer.add_polygon(poly);
    }

    layer.restore_graphics_state();

    Some(())
}

fn translate_poly(svg: &SvgPolygonInner, paintmode: PaintMode) -> printpdf::Polygon {
    printpdf::Polygon {
        rings: {
            let mut r = Vec::new();
            let points = svg.outer_ring.points.clone();
            r.push(
                points
                    .into_iter()
                    .map(|p| printpdf::Point {
                        x: Mm(p.x as f32).into_pt(),
                        y: Mm(p.y as f32).into_pt(),
                    })
                    .map(|p| (p, false))
                    .collect(),
            );
            for inner in svg.inner_rings.iter() {
                let mut points = inner.points.clone();
                points.reverse();
                r.push(
                    points
                        .into_iter()
                        .map(|p| printpdf::Point {
                            x: Mm(p.x as f32).into_pt(),
                            y: Mm(p.y as f32).into_pt(),
                        })
                        .map(|p| (p, false))
                        .collect(),
                );
            }
            r
        },
        mode: paintmode,
        winding_order: printpdf::path::WindingOrder::NonZero,
    }
}
