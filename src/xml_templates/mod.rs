use std::path::PathBuf;

pub const ANTRAGSBEGLEITBLATT_ZIP: &[u8] = include_bytes!("./Antragsbegleitblatt.zip");
pub const BEARBEITUNGSLISTE_ZIP: &[u8] = include_bytes!("./Bearbeitungsliste.zip");
pub const FORTFUEHRUNGSBELEG_ZIP: &[u8] = include_bytes!("./Fortfuehrungsbeleg.zip");

pub const ANTRAGSBEGLEITBLATT_DOCX_XML: &str = include_str!("./antragsbegleitblatt_document.xml");
pub const ANTRAGSBEGLEITBLATT_DOCX_FLURSTUECKE_XML: &str = include_str!("./antragsbegleitblatt_flurstuecke.xml");
pub const ANTRAGSBEGLEITBLATT_DOCX_ZEILE_XML: &str = include_str!("./antragsbegleitblatt_row.xml");

pub struct AntragsbegleitblattInfo {
    pub datum: String, // %%REPLACEME_DATUM%%
    pub antragsnr: String, // %%ANTRAGSNR_NUR_NUMMER%%
    pub gemarkung: String, // %%GEMARKUNGSNAME%%
    pub gemarkungsnummer: String, // %%GEMARKUNG_NUMMER%%
    pub fluren_bearbeitet: String, // %%FLUREN_NUMMERN%%
    pub flurstuecke_bearbeitet: Vec<(String, String)>, // <!-- %%FLURSTUECKE%% --> // Fl. X: Y
    pub eigentuemer: Vec<(String, Vec<String>)>, // <!-- %%ROWS%% --> // "Herr Soundso" -> { Fl. 1: Flst. 44, Fl. 2: Flst 55 }
}

pub fn generate_antragsbegleitblatt_docx(info: &AntragsbegleitblattInfo) -> Vec<(Option<String>, PathBuf, Vec<u8>)> {

    let document_xml = ANTRAGSBEGLEITBLATT_DOCX_XML
    .replace("<w:t>%%REPLACEME_DATUM%%</w:t>", &format!("<w:t>{}</w:t>", info.datum))
    .replace("<w:t>%%ANTRAGSNR_NUR_NUMMER%%</w:t>", &format!("<w:t>{}</w:t>", info.antragsnr))
    .replace("<w:t>%%GEMARKUNGSNAME%%</w:t>", &format!("<w:t>{}</w:t>", info.gemarkung))
    .replace("<w:t>%%GEMARKUNG_NUMMER%%</w:t>", &format!("<w:t>{}</w:t>", info.gemarkungsnummer))
    .replace("<w:t>%%FLUREN_NUMMERN%%</w:t>", &format!("<w:t>{}</w:t>", info.fluren_bearbeitet))
    .replace("<!-- %%FLURSTUECKE%% -->", &info.flurstuecke_bearbeitet.iter().map(antragsbegleitblatt_gen_bearbeitete_flst).collect::<Vec<_>>().join(""))
    .replace("<!-- %%ROWS%% -->", &info.eigentuemer.iter().map(antragsbegleitblatt_gen_row).collect::<Vec<_>>().join(""));

    let mut zip = crate::zip::read_files_from_zip(ANTRAGSBEGLEITBLATT_ZIP, true);
    for f in zip.iter() {
        crate::uuid_wasm::log_status(&format!("reading zip: got file {} (parent = {:?}): {} bytes", f.1.display(), f.0, f.2.len()));
    }
    zip.push((Some("word".to_string()), "document.xml".into(), document_xml.as_bytes().to_vec()));
    zip
    // crate::zip::write_files_to_zip(&zip)
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
    pub datum: String,

}
pub fn generate_bearbeitungsliste_xlsx(info: &BearbeitungslisteInfo) -> Vec<u8> {
    Vec::new()
}


pub struct FortfuehrungsbelegInfo {
    pub datum: String,

}

pub fn generate_fortfuehrungsbeleg_docx(info: &FortfuehrungsbelegInfo) -> Vec<u8> {
    Vec::new()
}
