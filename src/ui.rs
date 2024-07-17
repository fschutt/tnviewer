use std::collections::{BTreeMap, BTreeSet};

use serde_derive::{Serialize, Deserialize};

use crate::{csv::{CsvDataType, Status}, nas::{SplitNasXml, SvgPolygon}, search::NutzungsArt, xlsx::FlstIdParsed};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UiData {
    #[serde(default)]
    pub popover_state: Option<PopoverState>,
    #[serde(default)]
    pub tab: Option<usize>,
    #[serde(default)]
    pub tool: Option<Tool>,
    #[serde(default)]
    pub data_loaded: bool,
    #[serde(default)]
    pub selected_edit_flst: String,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Tool {
    #[serde(rename = "gebaeude-loeschen")]
    GebaeudeLoeschen,
    #[serde(rename = "nutzung-einzeichnen")]
    NutzungEinzeichnen,
}

impl UiData {

    pub fn from_string(s: &str) -> UiData {
        serde_json::from_str::<UiData>(s)
        .unwrap_or_default()
    }

    pub fn is_context_menu_open(&self) -> bool {
        match self.popover_state {
            Some(PopoverState::ContextMenu(_)) => true,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, PartialOrd, Clone)]
pub enum PopoverState {
    ContextMenu(ContextMenuData),
    Info,
    Configuration(ConfigurationView),
    Help,
}

#[test]
fn test1() {
    let s = serde_json::to_string(&PopoverState::Info).unwrap_or_default();
    println!("{s}");
}
#[derive(Debug, Copy, PartialEq, Serialize, Deserialize, PartialOrd, Clone)]
pub enum ConfigurationView {
    #[serde(rename = "allgemein")]
    Allgemein,
    #[serde(rename = "kartenstile")]
    Kartenstile,
}

#[derive(Debug, Copy, Serialize, Deserialize, PartialEq, PartialOrd, Clone)]
pub struct ContextMenuData {
    pub x: f32,
    pub y: f32,
}

// render entire <body> node depending on the state of the rpc_data
pub fn render_entire_screen(rpc_data: &UiData, csv: &CsvDataType, aenderungen: &Aenderungen) -> String {
    normalize_for_js(format!(
        "
            {popover}
            <div id='__application-ribbon'>
                {ribbon_ui}
            </div>
            <div id='__application-main' style='overflow:hidden;'>
                {main}
            </div>
        ",
        popover = render_popover(rpc_data),
        ribbon_ui = render_ribbon(rpc_data),
        main = render_main(rpc_data, csv, aenderungen),
    ))
}

pub fn render_popover(rpc_data: &UiData) -> String {
    let should_render_popover = rpc_data.popover_state.is_some();

    if !should_render_popover {
        return normalize_for_js(format!(
            "<div id='__application_popover' style='
            pointer-events:none;
            width: 100%;
            height: 100%;
            min-height: 100%;
            position: fixed;
            z-index:999;
        '></div>"
        ));
    }

    let popover = format!(
        "<div id='__application_popover' style='
        pointer-events:none;
        width: 100%;
        height: 100%;
        min-height: 100%;
        position: fixed;
        z-index:999;
    '>{}</div>",
        render_popover_content(rpc_data)
    );

    normalize_for_js(popover)
}

pub fn base64_encode<T: AsRef<[u8]>>(input: T) -> String {
    base64::encode(input)
}

pub fn render_popover_content(rpc_data: &UiData) -> String {
    const ICON_CLOSE: &[u8] = include_bytes!("./img/icons8-close-96.png");

    if rpc_data.popover_state.is_none() {
        return String::new();
    }
    
    let application_popover_color = if !rpc_data.is_context_menu_open() {
        "rgba(0, 0, 0, 0.5)"
    } else {
        "transparent"
    };

    let icon_close_base64 = base64_encode(ICON_CLOSE);

    let close_button = format!("
    <div style='position:absolute;top:50px;z-index:9999;right:-25px;background:white;border-radius:10px;box-shadow: 0px 0px 10px #cccccc88;cursor:pointer;' onmouseup='closePopOver()'>
        <img src='data:image/png;base64,{icon_close_base64}' style='width:50px;height:50px;cursor:pointer;' />
    </div>");

    let pc = match &rpc_data.popover_state {
        None => return String::new(),
        Some(PopoverState::Info) => {
            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:800px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                {close_button}

                <h2 style='font-size:24px;font-family:sans-serif;'>Digitales Projekt Version {version}</h2>
                
                <div style='padding:5px 0px;display:flex;flex-grow:1;min-height:750px;'>
                    <iframe width='auto' height='auto' src='data:text/html;base64,{license_base64}' style='min-width:100%;min-height:100%;'></iframe>                       
                </div>
                                
            </div>
            ",version = env!("CARGO_PKG_VERSION"),
            license_base64 = base64_encode(include_bytes!("../licenses.html")))
        }
        Some(PopoverState::Help) => {
            static DOKU: &str = include_str!("../doc/Handbuch.html");

            static IMG_1: &[u8] = include_bytes!("../doc/IMG_1.png");
            static IMG_2: &[u8] = include_bytes!("../doc/IMG_2.png");
            static IMG_3: &[u8] = include_bytes!("../doc/IMG_3.png");
            static IMG_4: &[u8] = include_bytes!("../doc/IMG_4.png");
            static IMG_5: &[u8] = include_bytes!("../doc/IMG_5.png");
            static IMG_6: &[u8] = include_bytes!("../doc/IMG_6.png");
            static IMG_7: &[u8] = include_bytes!("../doc/IMG_7.png");
            static IMG_8: &[u8] = include_bytes!("../doc/IMG_8.png");

            let base64_dok = base64_encode(
                DOKU.replace("$$DATA_IMG_1$$", &base64_encode(IMG_1))
                    .replace("$$DATA_IMG_2$$", &base64_encode(IMG_2))
                    .replace("$$DATA_IMG_3$$", &base64_encode(IMG_3))
                    .replace("$$DATA_IMG_4$$", &base64_encode(IMG_4))
                    .replace("$$DATA_IMG_5$$", &base64_encode(IMG_5))
                    .replace("$$DATA_IMG_6$$", &base64_encode(IMG_6))
                    .replace("$$DATA_IMG_7$$", &base64_encode(IMG_7))
                    .replace("$$DATA_IMG_8$$", &base64_encode(IMG_8)),
            );

            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:800px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>

                {close_button}
                
                <h2 style='font-size:24px;font-family:sans-serif;margin-bottom:25px;'>Benutzerhandbuch</h2>
                <div style='padding:5px 0px;display:flex;flex-grow:1;line-height:1.5;min-height:750px;'>
                    <iframe src='data:text/html;base64,{base64_dok}' width='100%' height='100%' style='min-width:100%;min-height:750px;display:flex;flex-grow:1;'/>
                </div>

            </div>")
        }
        Some(PopoverState::Configuration(cw)) => {
            use ConfigurationView::*;

            static IMG_SETTINGS: &[u8] =
                include_bytes!("./img/icons8-settings-system-daydream-96.png");
            let img_settings = base64_encode(IMG_SETTINGS);

            static IMG_REGEX: &[u8] = include_bytes!("./img/icons8-select-96.png");
            let img_regex = base64_encode(IMG_REGEX);

            static IMG_CLEAN: &[u8] = include_bytes!("./img/icons8-broom-96.png");
            let img_clean = base64_encode(IMG_CLEAN);

            static IMG_ABK: &[u8] = include_bytes!("./img/icons8-shortcut-96.png");
            let img_abk = base64_encode(IMG_ABK);

            static IMG_FX: &[u8] = include_bytes!("./img/icons8-formula-fx-96.png");
            let img_fx = base64_encode(IMG_FX);

            let active_allgemein = if *cw == Allgemein { " active" } else { "" };
            let active_kartenstile = if *cw == Kartenstile { " active" } else { "" };
 
            let sidebar = format!("
                <div class='__application_configuration_sidebar' style='display:flex;flex-direction:column;width:160px;min-height:750px;'>
                    
                    <div class='__application_configuration_sidebar_section{active_allgemein}' onmouseup='activateConfigurationView(event, \"allgemein\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_settings}'></img>
                        <p>Allgemein</p>
                    </div>
                    
                    <hr/>
                    
                    <div class='__application_configuration_sidebar_section{active_kartenstile}' onmouseup='activateConfigurationView(event, \"kartenstile\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>Kartenstile</p>
                    </div>

                </div>
            ");

            let main_content = match cw {
                Allgemein => format!("
                    <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                        <div>
                            <div style='display:flex;flex-direction:row;'>
                                <input style='width:20px;height:20px;cursor:pointer;' type='checkbox' id='__application_konfiguration_spalten_ausblenden' {spalten_einblenden} data-checkBoxId='konfiguration-spalten-ausblenden' onchange='toggleCheckbox(event)'>
                                <label style='font-size:20px;font-style:italic;' for='__application_konfiguration_spalten_ausblenden'>Formularspalten einblenden</label>
                            </div>
                            
                            <div style='display:flex;flex-direction:row;'>
                                <input style='width:20px;height:20px;cursor:pointer;' type='checkbox' id='__application_konfiguration_zeilenumbrueche-in-ocr-text' data-checkBoxId='konfiguration-zeilenumbrueche-in-ocr-text' {zeilenumbrueche_in_ocr_text} onchange='toggleCheckbox(event)'>
                                <label style='font-size:20px;font-style:italic;' for='__application_konfiguration_zeilenumbrueche-in-ocr-text'>Beim Kopieren von OCR-Text Zeilenumbrüche beibehalten</label>
                            </div>
                            
                            <div style='display:flex;flex-direction:row;'>
                                <input style='width:20px;height:20px;cursor:pointer;' type='checkbox' id='__application_konfiguration_hide_red_lines' data-checkBoxId='konfiguration-keine-roten-linien' {vorschau_ohne_geroetet} onchange='toggleCheckbox(event)'>
                                <label style='font-size:20px;font-style:italic;' for='__application_konfiguration_hide_red_lines'>PDF ohne geröteten Linien darstellen</label>
                            </div>
                        </div>
                        
                        <div style='margin-top:25px;'>
                            <h2 style='font-size:20px;'>Datenbank</h2>
                            
                            <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                <label style='font-size:20px;font-style:italic;'>Server-URL</label>
                                <input type='text' id='__application_konfiguration_datenbank_server' style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;min-width:300px;' value='{server_url}' data-konfiguration-textfield='server-url' onchange='editKonfigurationTextField(event)'></input>
                            </div>
                    
                            <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                <label style='font-size:20px;font-style:italic;'>E-Mail</label>
                                <input type='text' id='__application_konfiguration_datenbank_email' style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;min-width:300px;' value='{server_email}' data-konfiguration-textfield='email' onchange='editKonfigurationTextField(event)'></input>
                            </div>
                            
                            <div style='display:flex;flex-direction:row;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                <label style='font-size:20px;font-style:italic;'>Zertifikatsdatei</label>
                                <div style='width:200px;'><p>{cert_sig}</p></div>
                                <input type='file' class='btn btn_neu' id='__application_konfiguration_datenbank_private_key' onchange='editKonfigurationSchluesseldatei(event)' accept='.pfx'></input>
                                <input type='button' value='Datei auswählen...' class='btn btn_neu' data-file-input-id='__application_konfiguration_datenbank_private_key' onclick='document.getElementById(event.target.dataset.fileInputId).click();' />
                            </div>
                        </div>
                    </div>
                ",
                    server_url = String::new(), // rpc_data.konfiguration.server_url,
                    server_email = String::new(), // rpc_data.konfiguration.server_email,
                    cert_sig = String::new(), // rpc_data.konfiguration.get_cert().map(|cert| cert.fingerprint().to_spaced_hex()).unwrap_or_default(),
                    vorschau_ohne_geroetet = "", // if rpc_data.konfiguration.vorschau_ohne_geroetet { "checked" } else { "" },
                    spalten_einblenden = "", // if !rpc_data.konfiguration.spalten_ausblenden { "checked" } else { "" },
                    zeilenumbrueche_in_ocr_text = "", // if rpc_data.konfiguration.zeilenumbrueche_in_ocr_text { "checked" } else { "" },
                ),
                _ => String::new(),
            };

            let main = format!("<div style='display:flex;flex-grow:1;padding:0px 20px;line-height: 1.2;'>{main_content}</div>");

            format!("
                <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:1000px;position:relative;display:flex;flex-direction:column;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                    {close_button}
                    
                    <h2 style='font-size:24px;margin-bottom:15px;font-family:sans-serif;'>Konfiguration</h2>
                    <p style='font-size:12px;padding-bottom:10px;'>Pfad: {konfig_pfad}</p>
                    
                    <div style='display:flex;flex-direction:row;flex-grow:1;width:100%;'>
                        {sidebar}
                        {main}
                    </div>
                </div>
            ", 
                konfig_pfad = String::new(), // Konfiguration::konfiguration_pfad(),
            )
        }
        Some(PopoverState::ContextMenu(cm)) => {
            format!("
                <div style='pointer-events:unset;padding:1px;position:absolute;left:{}px;top:{}px;background:white;border-radius:5px;box-shadow:0px 0px 5px #444;'>
                    <div style='border:1px solid #efefef;border-radius:5px;'>
                        <p style='padding:5px 10px;font-size:10px;color:#444;margin-bottom:5px;'>Klassifiziere Seite als...</p>
                        <div style='line-height:1.5;cursor:pointer;'>
                            <div class='kontextmenü-eintrag' data-seite-neu='bv-horz' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Bestandsverzeichnis (Querformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='bv-horz-zu-und-abschreibungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Bestandsverzeichnis Zu- und Abschreibungen (Querformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='bv-vert' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Bestandsverzeichnis (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='bv-vert-typ2' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Bestandsverzeichnis Variante 2 (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='bv-vert-zu-und-abschreibungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Bestandsverzeichnis Zu- und Abschreibungen (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='bv-vert-zu-und-abschreibungen-alt' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Bestandsverzeichnis Zu- und Abschreibungen Variante 2(Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt1-horz' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 1 (Querformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt1-vert' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 1 (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt1-vert-typ2' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 1 Typ 2 (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt2-horz-veraenderungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 2 Veränderungen (Querformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt2-horz' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 2 (Querformat)
                            </div>

                            <div class='kontextmenü-eintrag' data-seite-neu='abt2-vert' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 2 (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt2-vert-typ2' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 2 Variante 2 (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt2-vert-veraenderungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 2 Veränderungen (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt3-horz-veraenderungen-loeschungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 3 Veränderungen / Löschungen (Querformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt3-horz' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 3 (Querformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt3-vert-veraenderungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                <p>Abteilung 3 Veränderungen (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt3-vert-loeschungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 3 Löschungen (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt3-vert-veraenderungen-loeschungen' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 3 Veränderungen / Löschungen (Hochformat)
                            </div>
                            <div class='kontextmenü-eintrag' data-seite-neu='abt3-vert' data-seite='{seite}' onmousedown='klassifiziereSeiteNeu(event);'>
                                Abteilung 3 (Hochformat)
                            </div>
                        </div>
                    </div>
                </div>", 
                cm.x, 
                cm.y,
                seite = 0, // cm.seite_ausgewaehlt
            )
        }
    };

    let pc = format!("
        <div style='background:{application_popover_color};width: 100%;height: 100%;min-height: 100%;z-index:1001;pointer-events:all;{overflow}' onmouseup='closePopOver()'>
            {pc}
        </div>", 
        overflow = if rpc_data.is_context_menu_open() { "" } else { "overflow:auto;" }, 
        application_popover_color = application_popover_color,
        pc = pc,
    );

    normalize_for_js(pc)
}

pub fn render_ribbon(rpc_data: &UiData) -> String {
    static ICON_EINSTELLUNGEN: &[u8] = include_bytes!("./img/icons8-settings-48.png");
    static ICON_HELP: &[u8] = include_bytes!("./img/icons8-help-96.png");
    static ICON_INFO: &[u8] = include_bytes!("./img/icons8-info-48.png");
    static ICON_GRUNDBUCH_OEFFNEN: &[u8] = include_bytes!("./img/icons8-book-96.png");
    static ICON_ZURUECK: &[u8] = include_bytes!("./img/icons8-back-48.png");
    static ICON_VORWAERTS: &[u8] = include_bytes!("./img/icons8-forward-48.png");
    static ICON_EXPORT_CSV: &[u8] = include_bytes!("./img/icons8-microsoft-excel-2019-96.png");
    static ICON_EXPORT_LEFIS: &[u8] = include_bytes!("./img/icons8-export-96.png");
    static ICON_DOWNLOAD: &[u8] = include_bytes!("./img/icons8-desktop-download-48.png");
    static ICON_DELETE: &[u8] = include_bytes!("./img/icons8-delete-trash-48.png");
    static ICON_PDF: &[u8] = include_bytes!("./img/icons8-pdf-48.png");
    static ICON_RECHTE_AUSGEBEN: &[u8] = include_bytes!("./img/icons8-scales-96.png");
    static ICON_FEHLER_AUSGEBEN: &[u8] = include_bytes!("./img/icons8-high-priority-96.png");
    static ICON_ABT1_AUSGEBEN: &[u8] = include_bytes!("./img/icons8-person-96.png");
    static ICON_TEILBELASTUNGEN_AUSGEBEN: &[u8] = include_bytes!("./img/icons8-pass-fail-96.png");
    static ICON_NEU: &[u8] = include_bytes!("./img/icons8-add-file-96.png");
    static ICON_SEARCH: &[u8] = include_bytes!("./img/icons8-search-in-cloud-96.png");
    static ICON_UPLOAD: &[u8] = include_bytes!("./img/icons8-upload-to-cloud-96.png");
    static ICON_HVM: &[u8] = include_bytes!("./img/icons8-copy-link-96.png");
    static RELOAD_PNG: &[u8] = include_bytes!("../src/img/icons8-synchronize-48.png");

    let disabled = if rpc_data.data_loaded {
        " disabled"
    } else {
        ""
    };
    let icon_open_base64 = base64_encode(ICON_GRUNDBUCH_OEFFNEN);
    let icon_neu_base64 = base64_encode(ICON_NEU);
    let icon_back_base64 = base64_encode(ICON_ZURUECK);
    let icon_forward_base64 = base64_encode(ICON_VORWAERTS);
    let icon_settings_base64 = base64_encode(ICON_EINSTELLUNGEN);
    let icon_help_base64 = base64_encode(ICON_HELP);
    let icon_info_base64 = base64_encode(ICON_INFO);
    let icon_export_pdf = base64_encode(ICON_PDF);
    let icon_rechte_speichern = base64_encode(ICON_RECHTE_AUSGEBEN);
    let icon_fehler_speichern = base64_encode(ICON_FEHLER_AUSGEBEN);
    let icon_export_teilbelastungen = base64_encode(ICON_TEILBELASTUNGEN_AUSGEBEN);
    let icon_export_abt1 = base64_encode(ICON_ABT1_AUSGEBEN);
    let icon_upload_lefis = base64_encode(ICON_UPLOAD);
    let icon_export_csv = base64_encode(ICON_EXPORT_CSV);
    let icon_export_lefis = base64_encode(ICON_EXPORT_LEFIS);
    let icon_hvm = base64_encode(ICON_HVM);
    let icon_download_base64 = base64_encode(ICON_DOWNLOAD);
    let icon_delete_base64 = base64_encode(ICON_DELETE);
    let icon_search_base64 = base64_encode(ICON_SEARCH);
    let icon_reload = base64_encode(&RELOAD_PNG);


    // TAB 1

    let projekt_oeffnen = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.load_project(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon' src='data:image/png;base64,{icon_open_base64}'>
                </div>
                <div>
                    <p>Projekt</p>
                    <p>laden</p>
                </div>
            </label>
        </div>
        ")
    };

    let neues_projekt = {
        format!("
        <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.create_project(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon' src='data:image/png;base64,{icon_neu_base64}'>
                    </div>
                    <div>
                        <p>Projekt</p>
                        <p>aus CSV</p>
                    </div>
                </label>
            </div>
        ")
    };

    let zurueck = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.undo(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_back_base64}'>
                </div>
                <div>
                    <p>Zurück</p>
                    <p>&nbsp;</p>
                </div>
            </label>
        </div>
        ")
    };

    let vorwaerts = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.redo(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_forward_base64}'>
                </div>
                <div>
                    <p>Vorwärts</p>
                    <p>&nbsp;</p>
                </div>
            </label>
        </div>
        ")
    };

    let daten_importieren = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.import_data(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_reload}'>
                </div>
                <div>
                    <p>NAS-Daten</p>
                    <p>importieren</p>
                </div>
            </label>
        </div>   
        ")
    };

    let projekt_speichern = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.save_project(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_upload_lefis}'>
                </div>
                <div>
                    <p>Änderungen</p>
                    <p>speichern</p>
                </div>
            </label>
        </div>
        ")
    };

    let einstellungen = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.open_configuration(event);' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon' src='data:image/png;base64,{icon_settings_base64}'>
                </div>
                <div>
                    <p>Einstellungen</p>
                    <p>bearbeiten</p>
                </div>
            </label>
        </div>
        ")
    };

    let hilfe = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.open_help(event);' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon' src='data:image/png;base64,{icon_help_base64}'>
                </div>
                <div>
                    <p>Hilfe</p>
                    <p>&nbsp;</p>
                </div>
            </label>
        </div>    
        ")
    };

    let info = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.open_info(event);' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon' src='data:image/png;base64,{icon_info_base64}'>
                </div>
                <div>
                    <p>Info</p>
                    <p>&nbsp;</p>
                </div>
            </label>
        </div>
        ")
    };

    // TAB 2

    let gebaeude_loeschen = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.gebaeude_loeschen(event)' class='__application-ribbon-action-vertical-large' style='{active}'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_delete_base64}'>
                    </div>
                    <div>
                        <p>Gebäude</p>
                        <p>löschen</p>
                    </div>
                </label>
            </div>
        ", active = match rpc_data.tool {
            Some(Tool::GebaeudeLoeschen) => "background:red !important;color:white !important;",
            _ => "",
        })
    };

    let nutzung_einzeichnen = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.nutzung_einzeichnen(event)' class='__application-ribbon-action-vertical-large' style='{active}'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_teilbelastungen}'>
                    </div>
                    <div>
                        <p>Nutzung</p>
                        <p>einzeichnen</p>
                    </div>
                </label>
            </div>
        ", active = match rpc_data.tool {
            Some(Tool::NutzungEinzeichnen) => "background:red !important;color:white !important;",
            _ => "",
        })
    };

    // TAB 3

    let export_excel = {
        format!("
            <div class='__application-ribbon-section 3'>
                <div style='display:flex;flex-direction:row;'>
                    <div class='__application-ribbon-section-content'>
                        <label onmouseup='tab_functions.export_excel(event)' class='__application-ribbon-action-vertical-large'>
                            <div class='icon-wrapper'>
                                <img class='icon {disabled}' src='data:image/png;base64,{icon_export_csv}'>
                            </div>
                            <div>
                                <p>Excel</p>
                                <p>exportieren</p>
                            </div>
                        </label>
                    </div>
                </div>
            </div>
        ")
    };

    let export_eigentuemer = {
        format!("
            <div class='__application-ribbon-section 3'>
                <div style='display:flex;flex-direction:row;'>
                    <div class='__application-ribbon-section-content'>
                        <label onmouseup='tab_functions.export_flst_nach_eigentuemer(event)' class='__application-ribbon-action-vertical-large'>
                            <div class='icon-wrapper'>
                                <img class='icon {disabled}' src='data:image/png;base64,{icon_export_csv}'>
                            </div>
                            <div>
                                <p>Bearb. Flst.</p>
                                <p>nach Eigentümer</p>
                            </div>
                        </label>
                    </div>
                </div>
            </div>
        ")
    };

    let export_pdf = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_pdf(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_export_pdf}'>
                </div>
                <div>
                    <p>Export</p>
                    <p>als PDF</p>
                </div>
            </label>
        </div>   
        ")
    };

    let export_alle_flurstuecke = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.export_alle_flst(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>Export</p>
                        <p>alle Flst.</p>
                    </div>
                </label>
            </div>
        ")
    };

    let export_veraenderte_flurstuecke = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.export_veraenderte_flst(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>Export</p>
                        <p>Flst. veränd.</p>
                    </div>
                </label>
            </div>
        ")
    };

    let export_david = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.export_david(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>Export</p>
                        <p>nach DAVID</p>
                    </div>
                </label>
            </div>
        ")
    };

    let ribbon_body = match rpc_data.tab.unwrap_or_default() {
        0 => {
            format!(
                "
            <div class='__application-ribbon-header'>
                <p onmouseup='selectTab(0);' class='active'>START</p>
                <p onmouseup='selectTab(1);'>KORREKTUR</p>
                <p onmouseup='selectTab(2);'>EXPORT</p>
                <div style='flex-grow:1;'></div>
                <input type='search' placeholder='Nutzungsarten durchsuchen...' style='margin-right:5px;margin-top:5px;min-width:300px;border:1px solid gray;max-height:25px;padding:5px;' oninput='searchNA(event);' onchange='searchNA(event);' onfocusout='closePopOver();'></input>
            </div>
            <div class='__application-ribbon-body'>
                <div class='__application-ribbon-section 1'>
                    <div style='display:flex;flex-direction:row;'>
                        
                        {projekt_oeffnen}

                        {neues_projekt}
                        
                    </div>
                </div>
            
                <div class='__application-ribbon-section 2'>
                    <div style='display:flex;flex-direction:row;'>

                        {zurueck}
                        
                        {vorwaerts}
                    </div>
                </div>
                
                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {daten_importieren}
                    </div>
                </div>

                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {projekt_speichern}
                    </div>
                </div>

                <div style='display:flex;flex-grow:1;'></div>
                
                <div class='__application-ribbon-section 6'>
                    <div style='display:flex;flex-direction:row;'>

                        {einstellungen}

                        {hilfe}

                        {info}

                    </div>
                </div>
            </div>
            "
            )
        },
        1 => {
            format!(
                "
            <div class='__application-ribbon-header'>
                <p onmouseup='selectTab(0);'>START</p>
                <p onmouseup='selectTab(1);' class='active'>KORREKTUR</p>
                <p onmouseup='selectTab(2);'>EXPORT</p>
                <div style='flex-grow:1;'></div>
                <input type='search' placeholder='Nutzungsarten durchsuchen...' style='margin-right:5px;margin-top:5px;min-width:300px;border:1px solid gray;max-height:25px;padding:5px;' oninput='searchNA(event);' onchange='searchNA(event);' onfocusout='closePopOver();'></input>
            </div>
            <div class='__application-ribbon-body'>

                <div class='__application-ribbon-section 2'>
                    <div style='display:flex;flex-direction:row;'>
                        {gebaeude_loeschen}
                        {nutzung_einzeichnen}
                    </div>
                </div>

                <div style='display:flex;flex-grow:1;'></div>
                
                <div class='__application-ribbon-section 6'>
                    <div style='display:flex;flex-direction:row;'>

                        {einstellungen}

                        {hilfe}

                        {info}

                    </div>
                </div>           
            </div>
            "
            )
        }
        _ => {
            format!(
                "
            <div class='__application-ribbon-header'>
                <p onmouseup='selectTab(0);'>START</p>
                <p onmouseup='selectTab(1);'>KORREKTUR</p>
                <p onmouseup='selectTab(2);' class='active'>EXPORT</p>
                <div style='flex-grow:1;'></div>
                <input type='search' placeholder='Nutzungsarten durchsuchen...' style='margin-right:5px;margin-top:5px;min-width:300px;border:1px solid gray;max-height:25px;padding:5px;' oninput='searchNA(event);' onchange='searchNA(event);' onfocusout='closePopOver();'></input>
            </div>
            <div class='__application-ribbon-body'>

                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_excel}
                        {export_eigentuemer}
                        {export_pdf}
                    </div>
                </div>

                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_alle_flurstuecke}
                        
                        {export_veraenderte_flurstuecke}
                    </div>
                </div>

                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_david}
                    </div>
                </div>

                <div style='display:flex;flex-grow:1;'></div>
                
                <div class='__application-ribbon-section 6'>
                    <div style='display:flex;flex-direction:row;'>

                        {einstellungen}

                        {hilfe}

                        {info}

                    </div>
                </div>
            </div>
        "
            )
        }
    };

    normalize_for_js(ribbon_body)
}

pub type FlstId = String; // 121180...
pub type GebauedeId = String; // DE...
pub type FlstPartId = String; // 121180...-121180... (intersection polygon between polygons)
pub type Kuerzel = String; // UV, WBF, GR, ...
pub type RingId = String; // DE...
pub type NewPolyId = String; // oudbvW0wu...
pub type NewRingId = String; // oudbvW0wu...

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct PolyNeu {
    pub poly: SvgPolygon,
    pub nutzung: Option<Kuerzel>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct NaAenderungen {
    pub alt: Option<Kuerzel>,
    pub neu: Option<Kuerzel>,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Aenderungen {
    pub gebaeude_loeschen: BTreeSet<GebauedeId>,
    pub na_definiert: BTreeMap<FlstPartId, NaAenderungen>,
    pub na_polygone_neu: BTreeMap<NewPolyId, PolyNeu>,
}

pub fn render_main(uidata: &UiData, csv: &CsvDataType, aenderungen: &Aenderungen) -> String {
    let map = format!("
        <div id='__application-main-container' style='display:flex;flex-grow:1;position:relative;overflow:hidden;'>
            <div id='__application_main-overlay-container' style='width:400px;max-width:400px;min-width:400px;display:flex;flex-grow:0;flex-direction:row;box-shadow:0px 0px 10px black;z-index:999;'>
                <div style='background:white;padding:20px;pointer-events:all;min-width:400px;box-shadow:0px 0px 10px black;'>
                    <div id='__application_project_content' class='csv-scrollbox' style='scrollbar-width:none;overflow: scroll;display: flex;flex-direction: column;max-height: 100%;'>{primary}</div>
                </div>
            </div>
            <div id='__application_secondary-overlay-container' style='display:{display_secondary};width:400px;max-width:400px;min-width:400px;flex-grow:0;flex-direction:row;box-shadow:0px 0px 10px black;z-index:999;'>
                <div style='background:white;padding:20px;pointer-events:all;min-width:400px;box-shadow:0px 0px 10px black;'>
                    <div id='__application_secondary_content' class='csv-scrollbox' style='scrollbar-width:none;overflow: scroll;display: flex;flex-direction: column;max-height: 100%;'>{secondary}</div>
                </div>
            </div>
            <div id='mapcontainer' style='display:flex;flex-grow:1;flex-direction:row;z-index:0;'>
                <div id='map' style='width:100%;height:100%;'></div>
            </div>
        </div>
    ",
        primary = render_project_content(csv, uidata, &SplitNasXml::default()),
        display_secondary = match uidata.tab {
            Some(1) => "flex",
            _ => "none",
        },
        secondary = render_secondary_content(&aenderungen),
    );
    normalize_for_js(map) // TODO
}

pub fn render_secondary_content(aenderungen: &Aenderungen) -> String {

    let mut html = "<div id='aenderungen-container'>".to_string();
    
    html += "<h2>Gebäude löschen</h2>";
    html += "<div id='zu-loeschende-gebaeude'>";
    for gebaeude_id in aenderungen.gebaeude_loeschen.iter() {
        html.push_str(&format!(
            "<div class='__application-aenderung-container' id='gebaeude-loeschen-{gebaeude_id}' data-gebaeude-id='{gebaeude_id}'>
                <div style='display:flex;'>
                    <p class='__application-zoom-to' onclick='zoomToGebaeudeLoeschen(event);' data-gebaeude-id='{gebaeude_id}'>[Karte]</p>
                    <p style='color: white;font-weight: bold;' data-gebaeude-id='{gebaeude_id}'>{gebaeude_id}</p>
                </div>
                <p class='__application-secondary-undo' onclick='gebaeudeLoeschenUndo(event);' data-gebaeude-id='{gebaeude_id}'>X</p>
            </div>"
        ));
    }
    html += "</div>";

    html += "<h2>Neue Nutzungen</h2>";
    html += "<div id='neue-na'>";
    for (new_poly_id, polyneu) in aenderungen.na_polygone_neu.iter() {
        let select_nutzung = render_select(&polyneu.nutzung, "changeSelectPolyNeu", &new_poly_id, "aendern-poly-neu");
        let kuerzel_alt = &polyneu.nutzung;
        html.push_str(&format!(
            "<div class='na-neu' id='na-neu-{new_poly_id}' data-new-poly-id='{new_poly_id}'>
                <p onclick='zoomToPolyNeu(event);' data-new-poly-id='{new_poly_id}'>Karte</p>
                {select_nutzung}
                <p class='undo' onclick='polyNeuUndo(event);' data-new-poly-id='{new_poly_id}'>X</p>
            </div>"
        ));
    }
    html += "</div>";

    html += "</div>";
    html 
}

pub fn render_select(selected: &Option<String>, function: &str, id: &str, html_id: &str) -> String {
    let map: BTreeMap<String, NutzungsArt> = include!(concat!(env!("OUT_DIR"), "/nutzung.rs"));
    let mut s = format!("<select id='{html_id}-{id}' onchange='{function}(event);' data-id='{id}'>");
    s.push_str(&format!("<option {selected} value='NOTDEFINED'>nicht defin.</option>", selected = if selected.is_none() { " selected='selected' " } else { "" }));
    for k in map.keys() {
        let selected = if selected.as_deref() == Some(k) { " selected='selected' " } else { "" };
        s.push_str(&format!("<option {selected}>{k}</option>"));
    }
    s += "</select>";
    s
}

pub fn render_project_content(csv: &CsvDataType, uidata: &UiData, split_fs: &SplitNasXml) -> String {

    let s = match uidata.tab {
        None | Some(0) => render_csv_editable(&csv, false, &uidata.selected_edit_flst, None),
        Some(1) => render_csv_editable(&csv, true, &uidata.selected_edit_flst, Some(split_fs)),
        Some(2) => {
            format!("
            <h2>Projekt <input type='text' value='aslkdadfa' placeholder='Projektname...'></input></h2>
            ")
        },
        _ => String::new(),
    };

    normalize_for_js(s)
}

fn render_csv_editable(
    csv: &CsvDataType, 
    filter_out_bleibt: bool, 
    selected_edit_flst: &str,
    split_fs: Option<&SplitNasXml>,
) -> String {

    let selected_edit_flst = selected_edit_flst.replace("_", "");

    csv.iter()
    .filter_map(|(k, v)| {
        if filter_out_bleibt && v.iter().any(|f| f.status == Status::Bleibt) {
            None
        } else {
            Some((k, v))
        }
    })
    .filter_map(|(k, v)| {
        let flstidparsed = FlstIdParsed::from_str(k).parse_num()?;
        let selected = if selected_edit_flst.is_empty() {
            false 
        } else {
            k.starts_with(&selected_edit_flst) 
        };
        Some(format!("
        <div class='csv-datensatz' id='csv_flst_{flst_id}' style='background: {background_col};padding: 10px;margin-bottom: 10px;border-radius: 5px;display: flex;flex-direction: column;{border}' ondblclick='focusFlst(event);' data-id='{flst_id}'>
            <h5 style='font-size: 18px;font-weight: bold;color: white;'  data-id='{flst_id}'>Fl. {flur_formatted} Flst. {flst_id_formatted}</h5>
            <p style='font-size: 16px;color: white;margin-bottom: 5px;'  data-id='{flst_id}'>{nutzungsart}</p>
            <input type='text' placeholder='Notiz...' value='{notiz_value}' oninput='changeNotiz(event);' onchange='changeNotiz(event);' data-id='{flst_id}' style='font-family: sans-serif;margin-bottom: 10px;width: 100%;padding: 3px;font-size:16px;'></input>
            <select style='font-size:16px;padding:5px;' onchange='changeStatus(event);' data-id='{flst_id}'>
                <option value='bleibt' {selected_bleibt}>Bleibt</option>
                <option value='aenderung-keine-benachrichtigung' {selected_kb}>Änderung (keine Benachrichtigung)</option>
                <option value='aenderung-mit-benachrichtigung' {selected_mb}>Änderung (mit Benachrichtigung)</option>
            </select>
            {split_nas}
        </div>",
        background_col = match v.get(0).map(|f| f.status).unwrap_or(Status::Bleibt) {
            Status::Bleibt => "#3e3e58",
            Status::AenderungKeineBenachrichtigung => "#ff9a5a",
            Status::AenderungMitBenachrichtigung => "#ff4545",
        },
        nutzungsart = v.get(0).map(|q| q.nutzung.clone()).unwrap_or_default(),
        flst_id = k,
        border = if selected {
            "border:1px solid red;"
        } else {
            "border:1px solid transparent;"
        },
        flur_formatted = flstidparsed.get_flur(),
        flst_id_formatted = flstidparsed.format_str(),
        notiz_value = v.get(0).map(|s| s.notiz.clone()).unwrap_or_default(),
        selected_bleibt = if v.get(0).map(|s| s.status.clone()) == Some(Status::Bleibt) { "selected='selected'" } else { "" },
        selected_kb = if v.get(0).map(|s| s.status.clone()) == Some(Status::AenderungKeineBenachrichtigung) { "selected='selected'" } else { "" },
        selected_mb = if v.get(0).map(|s| s.status.clone()) == Some(Status::AenderungMitBenachrichtigung) { "selected='selected'" } else { "" },
        split_nas = match split_fs.and_then(|sn| sn.flurstuecke_nutzungen.get(&flstidparsed.format_start_str())) {
            None => String::new(),
            Some(s) => {
                format!(
                    "<div class='nutzung-veraendern'>{}</div>", 
                    s.iter().filter_map(|tp| {
                        let ax_ebene = tp.attributes.get("AX_Ebene")?;
                        let ax_flurstueck = flstidparsed.format_start_str();
                        let cut_obj_id = tp.attributes.get("id")?;
                        Some(format!(
                            "<div><p>{ax_ebene}:</p>{}</div>", 
                            render_select(&None, "nutzungsArtAendern", &format!("{ax_flurstueck}:{ax_ebene}:{cut_obj_id}"), "nutzungsart-aendern")
                        ))
                    }).collect::<Vec<_>>().join("")
                )
            }
        }
    ))
    }).collect::<Vec<_>>().join("")
}

pub fn normalize_for_js(s: String) -> String {
    s.lines()
        .map(|s| s.trim().replace('`', "'"))
        .collect::<Vec<_>>()
        .join("")
}
