use std::collections::BTreeMap;

use printpdf::{Mm, CustomPdfConformance, PdfConformance, PdfDocument};
use serde_derive::{Deserialize, Serialize};
use crate::csv::CsvDataType;
use crate::nas::{NasXMLFile, SplitNasXml, SvgLine};
use crate::ui::Aenderungen;

pub type Risse = BTreeMap<String, RissConfig>;

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
    risse: &Risse, 
    csv: &CsvDataType, 
    xml: &NasXMLFile,
    aenderungen: &Aenderungen, 
    split_flurstuecke: &SplitNasXml,
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
        let (page, layer) = if i == 0 {
            (page1, layer1)
        } else {
            doc.add_page(Mm(rc.width_mm), Mm(rc.height_mm), ri)
        };
        let mut page = doc.get_page(page);
        let mut layer = page.get_layer(layer);

        let aenderungen_repro = reproject_aenderungen_into_pdf_space(
            &aenderungen,
            &rc,
        );
    
    
        let xml_reprojected = reproject_split_nas_xml_into_pdf_space(
            &xml,
            &rc,
        );
    
        let split_flurstuecke_repro = reproject_split_flurstuecke_into_pdf_space(
            &split_flurstuecke,
            &rc,
        );
    }

    doc.save_to_bytes().unwrap_or_default()
}

fn reproject_aenderungen_into_pdf_space(
    aenderungen: &Aenderungen,
    riss: &RissConfig,
) -> Aenderungen {
    aenderungen.clone() // TODO
}

fn reproject_split_nas_xml_into_pdf_space(
    input: &NasXMLFile,
    riss: &RissConfig,
) -> NasXMLFile {
    input.clone() // TODO
}

fn reproject_split_flurstuecke_into_pdf_space(
    input: &SplitNasXml,
    riss: &RissConfig,
) -> SplitNasXml {
    input.clone() // TODO 
}