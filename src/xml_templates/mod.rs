use std::{collections::{BTreeMap, BTreeSet}, path::PathBuf};

use crate::{geograf::FlstEigentuemer, xlsx::FlstIdParsedNumber};

pub const ANTRAGSBEGLEITBLATT_ZIP: &[u8] = include_bytes!("./Antragsbegleitblatt.zip");
pub const BEARBEITUNGSLISTE_ZIP: &[u8] = include_bytes!("./Bearbeitungsliste.zip");
pub const FORTFUEHRUNGSBELEG_ZIP: &[u8] = include_bytes!("./Fortfuehrungsbeleg.zip");

pub const ANTRAGSBEGLEITBLATT_DOCX_XML: &str = include_str!("./antragsbegleitblatt_document.xml");
pub const ANTRAGSBEGLEITBLATT_DOCX_FLURSTUECKE_XML: &str = include_str!("./antragsbegleitblatt_flurstuecke.xml");
pub const ANTRAGSBEGLEITBLATT_DOCX_ZEILE_XML: &str = include_str!("./antragsbegleitblatt_row.xml");

pub const BEARBEITUNGSLISTE_SHAREDSTRINGS_XML: &str = include_str!("./bearbeitungsliste_sharedstrings.xml");
pub const BEARBEITUNGSLISTE_SHEET1_XML: &str = include_str!("./bearbeitungsliste_sheet1.xml");
pub const BEARBEITUNGSLISTE_HEADER_XML: &str = include_str!("./bearbeitungsliste_header.xml");
pub const BEARBEITUNGSLISTE_ROW_XML: &str = include_str!("./bearbeitungsliste_row.xml");

pub struct AntragsbegleitblattInfo {
    pub datum: String, // %%REPLACEME_DATUM%%
    pub antragsnr: String, // %%ANTRAGSNR%%
    pub gemarkung: String, // %%GEMARKUNGSNAME%%
    pub gemarkungsnummer: String, // %%GEMARKUNG_NUMMER%%
    pub fluren_bearbeitet: String, // %%FLUREN_NUMMERN%%
    pub flurstuecke_bearbeitet: Vec<(String, String)>, // <!-- %%FLURSTUECKE%% --> // Fl. X: Y
    pub eigentuemer: Vec<(String, Vec<String>)>, // <!-- %%ROWS%% --> // "Herr Soundso" -> { Fl. 1: Flst. 44, Fl. 2: Flst 55 }
}

pub fn generate_antragsbegleitblatt_docx(info: &AntragsbegleitblattInfo) -> Vec<u8> {

    let document_xml = ANTRAGSBEGLEITBLATT_DOCX_XML
    .replace("<w:t>%%REPLACEME_DATUM%%</w:t>", &format!("<w:t>{}</w:t>", info.datum))
    .replace("<w:t>%%ANTRAGSNR%%</w:t>", &format!("<w:t>{}</w:t>", info.antragsnr))
    .replace("<w:t>%%GEMARKUNGSNAME%%</w:t>", &format!("<w:t>{}</w:t>", info.gemarkung))
    .replace("<w:t>%%GEMARKUNG_NUMMER%%</w:t>", &format!("<w:t>{}</w:t>", info.gemarkungsnummer))
    .replace("<w:t>%%FLUREN_NUMMERN%%</w:t>", &format!("<w:t>{}</w:t>", info.fluren_bearbeitet))
    .replace("<!-- %%FLURSTUECKE%% -->", &info.flurstuecke_bearbeitet.iter().map(antragsbegleitblatt_gen_bearbeitete_flst).collect::<Vec<_>>().join(""))
    .replace("<!-- %%ROWS%% -->", &info.eigentuemer.iter().map(antragsbegleitblatt_gen_row).collect::<Vec<_>>().join(""));

    let mut zip = crate::zip::read_files_from_zip(ANTRAGSBEGLEITBLATT_ZIP, true, &[".rels"]);
    zip.push((Some("word".to_string()), "document.xml".into(), document_xml.as_bytes().to_vec()));
    crate::zip::write_files_to_zip(&zip)
}

fn antragsbegleitblatt_gen_row((eigentuemer, flst): &(String, Vec<String>)) -> String {
    ANTRAGSBEGLEITBLATT_DOCX_ZEILE_XML
    .replace("<w:t>%tn1%</w:t>", &format!("<w:t>{eigentuemer}</w:t>"))
    .replace("<w:t>%%FLURSTUECKE%%</w:t>", &format!("<w:t>{}</w:t>", flst.join(", ")))
}

fn antragsbegleitblatt_gen_bearbeitete_flst((flur, flst): &(String, String)) -> String {
    ANTRAGSBEGLEITBLATT_DOCX_FLURSTUECKE_XML
    .replace("<w:t>%%FLUR%%</w:t>", &format!("<w:t>{flur}</w:t>"))
    .replace("<w:t>%%FLURSTUECKE%%</w:t>", &format!("<w:t>{flst}</w:t>"))
}

pub struct BearbeitungslisteInfo {
    pub auftragsnr: String,
    pub gemarkungsnr: String,
    pub fluren: String,
    pub eigentuemer: BTreeMap<FlstIdParsedNumber, FlstEigentuemer>
}

pub fn generate_bearbeitungsliste_xlsx(info: &BearbeitungslisteInfo) -> Vec<u8> {

    let mut sharedstrings = BTreeSet::new();

    sharedstrings.extend(vec![
        format!("TN Bestand: Flurkarte    "),
        format!("TN Veränderung: GIS"),
        format!("Riss"),
        format!("David-Bearbeitung"),
        format!("Lage &amp; Bemerkung zur Bearbeitung"),
        format!("Flurstückseigentümer"),
        format!("To do Liste"),
        format!("Der Vergleich der Flurkarte mit der Örtlichkeit über GIS (Luftbild v. 2021-2023 + Landwirtschaftsfeldblöcke v. 11.02.2024)"),
        format!("Flurstückskennz."),
        format!("Auftragsnummer: {}, {}, Flur {}", info.auftragsnr, info.gemarkungsnr, info.fluren),
    ].into_iter());

    for (flst_id, v) in info.eigentuemer.iter() {
        let eig: String = v.eigentuemer.join("; ");
        let row_strings = vec![
            flst_id.format_nice(),
            v.nutzung.to_string(),
            match v.status {
                crate::csv::Status::Bleibt => "bleibt".to_string(),
                crate::csv::Status::AenderungKeineBenachrichtigung => v.auto_notiz.clone() + " (keine Benachrichtigung)",
                crate::csv::Status::AenderungMitBenachrichtigung => v.auto_notiz.clone() + " (mit Benachrichtigung)",
            },
            eig.to_string(),
            v.notiz.clone()
        ];
        for r in row_strings.into_iter() {
            sharedstrings.insert(r);
        }
    }

    let sharedstrings_list = sharedstrings.iter().cloned().collect::<Vec<_>>();
    let sharedstrings_lookup_list = sharedstrings_list.iter().cloned().enumerate().map(|(k, v)| (v, k)).collect::<BTreeMap<_, _>>();

    let sharedstrings_xml = BEARBEITUNGSLISTE_SHAREDSTRINGS_XML
    .replace("%%SHARED_STRINGS_COUNT%%", &sharedstrings.len().to_string())
    .replace("<!-- %%SHARED_STRINGS%% -->", &sharedstrings_list.iter().map(|s| format!("<si><t xml:space=\"preserve\">{s}</t></si>")).collect::<Vec<_>>().join(""));
    
    let mut bearbeitungsliste_rows = Vec::new();

    for (i, (flst_id, v)) in info.eigentuemer.iter().enumerate() {
        let eig: String = v.eigentuemer.join("; ");
        let row_style_id = match v.status {
            crate::csv::Status::Bleibt => "0",
            crate::csv::Status::AenderungKeineBenachrichtigung => "2",
            crate::csv::Status::AenderungMitBenachrichtigung => "3",
        };

        let row_strings = vec![
            flst_id.format_nice(),
            v.nutzung.to_string(),
            match v.status {
                crate::csv::Status::Bleibt => "bleibt".to_string(),
                crate::csv::Status::AenderungKeineBenachrichtigung => v.auto_notiz.clone() + " (keine Benachrichtigung)",
                crate::csv::Status::AenderungMitBenachrichtigung => v.auto_notiz.clone() + " (mit Benachrichtigung)",
            },
            eig.to_string(),
            v.notiz.clone()
        ];

        let mut row_xml = BEARBEITUNGSLISTE_ROW_XML
        .replace("%%ROWID%%", &(i + 5).to_string())
        .replace("%%CUSTOMFORMAT%%", &row_style_id);
    
        for (i, r) in row_strings.into_iter().enumerate() {
            let replaceid = format!("%%COL{i}%%");
            let string_id = match sharedstrings_lookup_list.get(&r) {
                Some(s) => s.to_string(),
                None => String::new(),
            };
            row_xml = row_xml.replace(&replaceid, &string_id);
        }
        bearbeitungsliste_rows.push(row_xml);
    }

    let sheet1_xml = BEARBEITUNGSLISTE_SHEET1_XML
    .replace("<!-- %%HEADER%% -->", &BEARBEITUNGSLISTE_HEADER_XML)
    .replace("<!-- %%ROWS%% -->", &bearbeitungsliste_rows.join(""));

    let mut zip = crate::zip::read_files_from_zip(BEARBEITUNGSLISTE_ZIP, true, &[".rels"]);
    zip.push((Some("xl".to_string()), "sharedStrings.xml".into(), sharedstrings_xml.as_bytes().to_vec()));
    zip.push((Some("xl/worksheets".to_string()), "sheet1.xml".into(), sheet1_xml.as_bytes().to_vec()));
    crate::zip::write_files_to_zip(&zip)
}


pub struct FortfuehrungsbelegInfo {
    pub datum: String,

}

pub fn generate_fortfuehrungsbeleg_docx(info: &FortfuehrungsbelegInfo) -> Vec<u8> {
    Vec::new()
}
