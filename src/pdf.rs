use std::collections::BTreeMap;

use printpdf::{Mm, CustomPdfConformance, PdfConformance, PdfDocument};
use serde_derive::{Deserialize, Serialize};
use crate::csv::CsvDataType;
use crate::nas::{NasXMLFile, SvgLine};

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RissConfig {
    pub title: String,
    pub center_lat: f64,
    pub center_lon: f64,
    pub target_crs: String,
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
pub fn generate_pdf(csv: &CsvDataType, xml: &NasXMLFile) -> Vec<u8> {

    let riss = RissConfig {
        title: "Riss1".to_string(),
        center_lat: 50.0,
        center_lon: 13.0,
        target_crs: "+proj=utm +ellps=GRS80 +units=m +no_defs +zone=33".to_string(),
        width_mm: 250.0,
        height_mm: 210.0,
        scale: 3500.0,
    };

    let (mut doc, page1, layer1) = PdfDocument::new(
        &riss.title,
        Mm(riss.width_mm),
        Mm(riss.height_mm),
        &riss.title,
    );

    doc = doc.with_conformance(PdfConformance::Custom(CustomPdfConformance {
        requires_icc_profile: false,
        requires_xmp_metadata: false,
        .. Default::default()
    }));

    doc.save_to_bytes().unwrap_or_default()
}