use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UiData {
    #[serde(default)]
    pub popover_state: Option<PopoverState>,
    #[serde(default)]
    pub tab: Option<usize>,
    #[serde(default)]
    pub data_loaded: bool,
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
    ExportPdf,
    CreateNewProjekt,
    ProjektMetaAendern {
        grundbuch_von: String,
        amtsgericht: String,
        blatt: String,
    },
    ProjektSuchenDialog,
    ProjektUploadDialog(usize),
    Configuration(ConfigurationView),
    Help,
}

#[derive(Debug, Copy, PartialEq, Serialize, Deserialize, PartialOrd, Clone)]
pub enum ConfigurationView {
    Allgemein,
    RegEx,
    TextSaubern,
    Abkuerzungen,
    FlstAuslesen,
    KlassifizierungRechteArt,
    RechtsinhaberAuslesenAbt2,
    RangvermerkAuslesenAbt2,
    TextKuerzenAbt2,
    BetragAuslesenAbt3,
    KlassifizierungSchuldenArtAbt3,
    RechtsinhaberAuslesenAbt3,
    TextKuerzenAbt3,
}

#[derive(Debug, Copy, Serialize, Deserialize, PartialEq, PartialOrd, Clone)]
pub struct ContextMenuData {
    pub x: f32,
    pub y: f32,
}

// render entire <body> node depending on the state of the rpc_data
pub fn render_entire_screen(rpc_data: &UiData) -> String {
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
        main = render_main(rpc_data),
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


pub fn render_popover_content(rpc_data: &UiData) -> String {
    const ICON_CLOSE: &[u8] = include_bytes!("./img/icons8-close-96.png");

    let application_popover_color = if !rpc_data.is_context_menu_open() {
        "rgba(0, 0, 0, 0.5)"
    } else {
        "transparent"
    };

    let icon_close_base64 = base64::encode(ICON_CLOSE);

    let close_button = format!("
    <div style='position:absolute;top:50px;z-index:9999;right:-25px;background:white;border-radius:10px;box-shadow: 0px 0px 10px #cccccc88;cursor:pointer;' onmouseup='closePopOver()'>
        <img src='data:image/png;base64,{icon_close_base64}' style='width:50px;height:50px;cursor:pointer;' />
    </div>");

    let pc = match &rpc_data.popover_state {
        None => return String::new(),
        Some(PopoverState::ProjektUploadDialog(i)) => {
            /*
            let commit_title = if rpc_data.commit_title.is_empty() {
                String::new()
            } else {
                format!("value='{}'", rpc_data.commit_title)
            };

            let commit_description = if rpc_data.commit_msg.is_empty() {
                String::new()
            } else {
                rpc_data
                    .commit_msg
                    .lines()
                    .map(|l| format!("<p>{l}</p>"))
                    .collect::<Vec<_>>()
                    .join("\r\n")
            };
            */
            let commit_title = String::new();
            let commit_description = String::new();
            let dateien = String::new();
            let diff = String::new();

            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:1200px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                {close_button}

                <h2 style='font-size:24px;font-family:sans-serif;margin-bottom:25px;'>Änderungen in Datenbank hochladen</h2>
                
                <div style='padding:5px 0px;display:flex;flex-grow:1;flex-direction:column;'>
                    <form onsubmit='grundbuchHochladen(event)' action=''>
                    
                    <div style='display:flex;font-size:16px;flex-direction:column;'>
                        <p style='font-size:16px;line-height:2;'>Beschreiben Sie ihre Änderungen:</p>
                        <input type='text' oninput='editCommitTitle(event);' id='__application_grundbuch_aenderung_commit_titel' required placeholder='z.B. \"Korrektur aufgrund von Kaufvertrag XXX/XXXX\"' style='font-size:18px;font-family:monospace;font-weight:bold;border:1px solid #ccc;cursor:text;display:flex;flex-grow:1;' {commit_title}></input>
                    </div>
                    
                    <div style='display:flex;font-size:16px;flex-direction:column;'>
                        <p style='font-size:16px;line-height:2;'>Ausführliche Beschreibung der Änderung:</p>
                        
                        <div style='display:flex;flex-grow:1;flex-direction:column;background:white;border:1px solid #efefef;margin-top:5px;font-weight:bold;font-size:14px;font-family:monospace;color:black;padding:0px;min-height:200px;max-height:250px;overflow-y:scroll;'>
                            <div style='padding-left:2px;caret-color: #4a4e6a;' contenteditable='true' onkeydown='insertTabAtCaret(event);' oninput='editCommitDescription(event);' id='__application_grundbuch_aenderung_commit_description'>{commit_description}</div>
                        </div>
                    </div>
                    
                    <div id='__application_grundbuch_upload_aenderungen' style='display:flex;flex-direction:row;min-height:300px;max-height:400px;flex-grow:1;overflow-y:scroll;'>
                        <div id='__application_aenderung_dateien' style='padding: 10px 0px;margin-right:10px;overflow-y: scroll;height: 300px;min-width: 300px;'>
                            {dateien}
                        </div>
                        <div id='__application_aenderungen_diff'>
                            {diff}
                        </div>
                    </div>
                    
                    <div style='display:flex;flex-direction:row;justify-content: flex-end;margin-top: 20px;'>
                        <input type='submit' value='Änderungen übernehmen' class='btn btn_neu' style='cursor:pointer;font-size:20px;height:unset;display:inline-block;flex-grow:0;max-width:320px;' />
                    </div>
                    </form>
                </div>
            </div>
            ")
        }
        Some(PopoverState::ProjektSuchenDialog) => {
            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:1000px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                {close_button}

                <h2 style='font-size:24px;font-family:sans-serif;'>Projektblatt suchen</h2>
                
                <div style='padding:5px 0px;display:flex;flex-grow:1;flex-direction:column;'>
                    <form onsubmit='grundbuchSuchen(event)' action=''>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;flex-direction:row;margin-bottom:20px;'>
                        <input type='text' id='__application_grundbuch_suchen_suchbegriff' required placeholder='Suchbegriff (z.B. \"Ludwigsburg Blatt 10\" oder \"Max Mustermann\")' style='font-size:14px;font-weight:bold;border-bottom:1px solid black;cursor:text;display:flex;flex-grow:1;'></input>
                        <input type='submit' value='Suchen' class='btn btn_neu' style='cursor:pointer;font-size:20px;height:unset;display:flex;flex-grow:0;margin-left:20px;' />
                        </div>
                    </form>
                    
                    <div id='__application_grundbuch_suchen_suchergebnisse' style='display:flex;flex-grow:1;min-height:500px;flex-direction:column;max-height:700px;overflow-y:scroll;'>
                    </div>
                </div>
            </div>
            ")
        }
        Some(PopoverState::CreateNewProjekt) => {
            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:800px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                {close_button}

                <h2 style='font-size:24px;font-family:sans-serif;margin-bottom:25px;'>Neues Projektblatt anlegen</h2>
                
                <div style='padding:5px 0px;display:flex;flex-grow:1;flex-direction:column;'>
                    <form onsubmit='grundbuchAnlegen(event)' action=''>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Amtsgericht</label>
                        <input type='text' id='__application_grundbuch_anlegen_amtsgericht' required style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;'></input>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Projekt von</label>
                        <input type='text' id='__application_grundbuch_anlegen_grundbuch_von' required style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;'></input>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Blatt-Nr.</label>
                        <input type='number' id='__application_grundbuch_anlegen_blatt_nr' required style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;'></input>
                    </div>
                    <br/>
                    <input type='submit' value='Speichern' class='btn btn_neu' style='cursor:pointer;font-size:20px;height:unset;display:inline-block;flex-grow:0;max-width:320px;margin-top:20px;' />
                    </form>
                </div>
            </div>
            ")
        },
        Some(PopoverState::ProjektMetaAendern { amtsgericht, grundbuch_von, blatt }) => {
            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:800px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                {close_button}

                <h2 style='font-size:24px;font-family:sans-serif;margin-bottom:25px;'>Neues Projektblatt anlegen</h2>
                
                <div style='padding:5px 0px;display:flex;flex-grow:1;flex-direction:column;'>
                    <form onsubmit='grundbuchMetaAendernFinished(event)' action=''>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Amtsgericht</label>
                        <input type='text' id='__application_grundbuch_anlegen_amtsgericht' required style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;' value='{amtsgericht}'></input>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Projekt von</label>
                        <input type='text' id='__application_grundbuch_anlegen_grundbuch_von' required style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;' value='{grundbuch_von}'></input>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Blatt-Nr.</label>
                        <input type='number' id='__application_grundbuch_anlegen_blatt_nr' required style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:text;' value='{blatt}'></input>
                    </div>
                    <br/>
                    <input type='submit' value='Speichern' class='btn btn_neu' style='cursor:pointer;font-size:20px;height:unset;display:inline-block;flex-grow:0;max-width:320px;margin-top:20px;' />
                    </form>
                </div>
            </div>
            ")
        }
        Some(PopoverState::ExportPdf) => {
            format!("
            <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:800px;display:flex;flex-direction:column;position:relative;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                {close_button}

                <h2 style='font-size:24px;font-family:sans-serif;margin-bottom:25px;'>PDF-Export</h2>
                
                <div style='padding:5px 0px;display:flex;flex-grow:1;flex-direction:column;'>
                    <form onsubmit='grundbuchExportieren(event)'  action=''>
                    
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Exportiere:</label>
                        
                        <select id='__application_export-pdf-was-exportieren' style='font-size:20px;font-weight:bold;border-bottom:1px solid black;cursor:pointer;'>
                            <option value='offen'>Offenes Projekt</option>
                            <option value='alle-offen-digitalisiert'>Alle offenen, digitalisierten Grundbücher</option>
                            <option value='alle-offen'>Alle offenen Grundbücher</option>
                            <option value='alle-original'>Alle Original-PDFs</option>
                        </select>
                    </div>

                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label style='font-size:20px;font-style:italic;'>Exportiere Abteilungen:</label>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label for='export-pdf-bv' style='font-size:16px;margin-left:10px;'>Bestandsverzeichnis</label>
                        <input id='export-pdf-bv' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label for='export-pdf-abt-1' style='font-size:16px;margin-left:10px;'>Abteilung 1</label>
                        <input id='export-pdf-abt-1' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label for='export-pdf-abt-2' style='font-size:16px;margin-left:10px;'>Abteilung 2</label>
                        <input id='export-pdf-abt-2' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <label for='export-pdf-abt-3' style='font-size:16px;margin-left:10px;'>Abteilung 3</label>
                        <input id='export-pdf-abt-3' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>
                    </div>
                    <br/>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <input id='export-pdf-leere-seite' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>                        
                        <label for='export-pdf-leere-seite' style='font-size:20px;font-style:italic;'>Leere Seite nach Titelblatt einfügen</label>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <input id='export-pdf-geroetete-eintraege' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>                        
                        <label for='export-pdf-geroetete-eintraege' style='font-size:20px;font-style:italic;'>Gerötete Einträge ausgeben</label>
                    </div>
                    <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                        <input id='export-pdf-eine-datei' type='checkbox' style='width:20px;height:20px;cursor:pointer;' checked='checked'/>                        
                        <label for='export-pdf-eine-datei' style='font-size:20px;font-style:italic;'>Als ein PDF ausgeben</label>
                    </div>
                    <input type='submit' value='Speichern' class='btn btn_neu' style='cursor:pointer;font-size:20px;height:unset;display:inline-block;flex-grow:0;max-width:320px;margin-top:20px;' />
                        
                    </form>
                </div>
            </div>
            ")
        }
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
            license_base64 = base64::encode(include_bytes!("../licenses.html")))
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

            let base64_dok = base64::encode(
                DOKU.replace("$$DATA_IMG_1$$", &base64::encode(IMG_1))
                    .replace("$$DATA_IMG_2$$", &base64::encode(IMG_2))
                    .replace("$$DATA_IMG_3$$", &base64::encode(IMG_3))
                    .replace("$$DATA_IMG_4$$", &base64::encode(IMG_4))
                    .replace("$$DATA_IMG_5$$", &base64::encode(IMG_5))
                    .replace("$$DATA_IMG_6$$", &base64::encode(IMG_6))
                    .replace("$$DATA_IMG_7$$", &base64::encode(IMG_7))
                    .replace("$$DATA_IMG_8$$", &base64::encode(IMG_8)),
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
            let img_settings = base64::encode(IMG_SETTINGS);

            static IMG_REGEX: &[u8] = include_bytes!("./img/icons8-select-96.png");
            let img_regex = base64::encode(IMG_REGEX);

            static IMG_CLEAN: &[u8] = include_bytes!("./img/icons8-broom-96.png");
            let img_clean = base64::encode(IMG_CLEAN);

            static IMG_ABK: &[u8] = include_bytes!("./img/icons8-shortcut-96.png");
            let img_abk = base64::encode(IMG_ABK);

            static IMG_FX: &[u8] = include_bytes!("./img/icons8-formula-fx-96.png");
            let img_fx = base64::encode(IMG_FX);

            let active_allgemein = if *cw == Allgemein { " active" } else { "" };
            let active_regex = if *cw == RegEx { " active" } else { "" };
            let active_text_saubern = if *cw == TextSaubern { " active" } else { "" };
            let active_abkuerzungen = if *cw == Abkuerzungen { " active" } else { "" };
            let active_flst_auslesen = if *cw == FlstAuslesen { " active" } else { "" };
            let active_klassifizierung_rechteart = if *cw == KlassifizierungRechteArt {
                " active"
            } else {
                ""
            };
            let active_rechtsinhaber_auslesen_abt2 = if *cw == RechtsinhaberAuslesenAbt2 {
                " active"
            } else {
                ""
            };
            let active_rangvermerk_auslesen_abt2 = if *cw == RangvermerkAuslesenAbt2 {
                " active"
            } else {
                ""
            };
            let active_text_kuerzen_abt2 = if *cw == TextKuerzenAbt2 { " active" } else { "" };
            let active_betrag_auslesen_abt3 = if *cw == BetragAuslesenAbt3 {
                " active"
            } else {
                ""
            };
            let active_klassifizierung_schuldenart_abt3 = if *cw == KlassifizierungSchuldenArtAbt3 {
                " active"
            } else {
                ""
            };
            let active_rechtsinhaber_auslesen_abt3 = if *cw == RechtsinhaberAuslesenAbt3 {
                " active"
            } else {
                ""
            };
            let active_text_kuerzen_abt3 = if *cw == TextKuerzenAbt3 { " active" } else { "" };

            let sidebar = format!("
                <div class='__application_configuration_sidebar' style='display:flex;flex-direction:column;width:160px;min-height:750px;'>
                    
                    <div class='__application_configuration_sidebar_section{active_allgemein}' onmouseup='activateConfigurationView(event, \"allgemein\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_settings}'></img>
                        <p>Allgemein</p>
                    </div>
                    
                    <hr/>
                    
                    <div class='__application_configuration_sidebar_section{active_regex}' onmouseup='activateConfigurationView(event, \"regex\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_regex}'></img>
                        <p>Reguläre Ausdrücke</p>
                    </div>
                    
                    <div class='__application_configuration_sidebar_section{active_text_saubern}' onmouseup='activateConfigurationView(event, \"text-saubern\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>Text säubern</p>
                    </div>
                    
                    <div class='__application_configuration_sidebar_section{active_abkuerzungen}' onmouseup='activateConfigurationView(event, \"abkuerzungen\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_abk}'></img>
                        <p>Abkürzungen</p>
                    </div>
                    
                    <div class='__application_configuration_sidebar_section{active_flst_auslesen}' onmouseup='activateConfigurationView(event, \"flst-auslesen\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Flurstücke auslesen</p>
                    </div>
                    
                    <hr/>

                    <div class='__application_configuration_sidebar_section{active_klassifizierung_rechteart}' onmouseup='activateConfigurationView(event, \"klassifizierung-rechteart-abt2\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Klassifizierung RechteArt (Abt. 2)</p>
                    </div>
                    
                    <div class='__application_configuration_sidebar_section{active_rechtsinhaber_auslesen_abt2}' onmouseup='activateConfigurationView(event, \"rechtsinhaber-auslesen-abt2\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Rechtsinhaber auslesen (Abt. 2)</p>
                    </div>
                    
                    <div class='__application_configuration_sidebar_section{active_rangvermerk_auslesen_abt2}' onmouseup='activateConfigurationView(event, \"rangvermerk-auslesen-abt2\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Rangvermerk auslesen (Abt. 2)</p>
                    </div>
                    
                    <div class='__application_configuration_sidebar_section{active_text_kuerzen_abt2}' onmouseup='activateConfigurationView(event, \"text-kuerzen-abt2\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Text kürzen (Abt. 2)</p>
                    </div>
                    
                    <hr/>

                    <div class='__application_configuration_sidebar_section{active_betrag_auslesen_abt3}' onmouseup='activateConfigurationView(event, \"betrag-auslesen-abt3\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Betrag auslesen (Abt. 3)</p>
                    </div>
                    <div class='__application_configuration_sidebar_section{active_klassifizierung_schuldenart_abt3}' onmouseup='activateConfigurationView(event, \"klassifizierung-schuldenart-abt3\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Klassifizierung SchuldenArt (Abt. 3)</p>
                    </div>
                    <div class='__application_configuration_sidebar_section{active_rechtsinhaber_auslesen_abt3}' onmouseup='activateConfigurationView(event, \"rechtsinhaber-auslesen-abt3\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Rechtsinhaber auslesen (Abt. 3)</p>
                    </div>
                    <div class='__application_configuration_sidebar_section{active_text_kuerzen_abt3}' onmouseup='activateConfigurationView(event, \"text-kuerzen-abt3\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_fx}'></img>
                        <p>Text kürzen (Abt. 3)</p>
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

    let disabled = if rpc_data.data_loaded {
        " disabled"
    } else {
        ""
    };
    let icon_open_base64 = base64::encode(ICON_GRUNDBUCH_OEFFNEN);
    let icon_neu_base64 = base64::encode(ICON_NEU);
    let icon_back_base64 = base64::encode(ICON_ZURUECK);
    let icon_forward_base64 = base64::encode(ICON_VORWAERTS);
    let icon_settings_base64 = base64::encode(ICON_EINSTELLUNGEN);
    let icon_help_base64 = base64::encode(ICON_HELP);
    let icon_info_base64 = base64::encode(ICON_INFO);
    let icon_download_base64 = base64::encode(ICON_DOWNLOAD);
    let icon_delete_base64 = base64::encode(ICON_DELETE);
    let icon_export_pdf = base64::encode(ICON_PDF);
    let icon_rechte_speichern = base64::encode(ICON_RECHTE_AUSGEBEN);
    let icon_fehler_speichern = base64::encode(ICON_FEHLER_AUSGEBEN);
    let icon_export_teilbelastungen = base64::encode(ICON_TEILBELASTUNGEN_AUSGEBEN);
    let icon_export_abt1 = base64::encode(ICON_ABT1_AUSGEBEN);
    let icon_search_base64 = base64::encode(ICON_SEARCH);
    let icon_upload_lefis = base64::encode(ICON_UPLOAD);
    let icon_export_csv = base64::encode(ICON_EXPORT_CSV);
    let icon_export_lefis = base64::encode(ICON_EXPORT_LEFIS);
    let icon_hvm = base64::encode(ICON_HVM);

    let nebenbet = {
        format!("
            <div class='__application-ribbon-section 3'>
                <div style='display:flex;flex-direction:row;'>
                    <div class='__application-ribbon-section-content'>
                        <label onmouseup='tab_functions.export_nb(event)' class='__application-ribbon-action-vertical-large'>
                            <div class='icon-wrapper'>
                                <img class='icon {disabled}' src='data:image/png;base64,{icon_export_csv}'>
                            </div>
                            <div>
                                <p>Nebenbet.</p>
                                <p>in CSV</p>
                            </div>
                        </label>
                    </div>
                    
                    <div class='__application-ribbon-section-content'>
                        <label onmouseup='tab_functions.import_nb(event)' class='__application-ribbon-action-vertical-large'>
                            <div class='icon-wrapper'>
                                <img class='icon {disabled}' src='data:image/png;base64,{icon_download_base64}'>
                            </div>
                            <div>
                                <p>Nebenbet.</p>
                                <p>importieren</p>
                            </div>
                        </label>
                    </div>
                    
                    <div class='__application-ribbon-section-content'>
                        <label onmouseup='tab_functions.delete_nb(event)' class='__application-ribbon-action-vertical-large'>
                            <div class='icon-wrapper'>
                                <img class='icon {disabled}' src='data:image/png;base64,{icon_delete_base64}'>
                            </div>
                            <div>
                                <p>Nebenbet.</p>
                                <p>entfernen</p>
                            </div>
                        </label>
                    </div>
                </div>
            </div>
        ")
    };

    let export_lefis = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.export_lefis(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>Export</p>
                        <p>(.lefis)</p>
                    </div>
                </label>
            </div>
        ")
    };

    let grundbuch_oeffnen = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.load_new_pdf(event)' class='__application-ribbon-action-vertical-large'>
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

    let neues_grundbuch = {
        format!("
        <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.create_new_grundbuch(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon' src='data:image/png;base64,{icon_neu_base64}'>
                    </div>
                    <div>
                        <p>Neues</p>
                        <p>Projekt</p>
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

    let alle_rechte_speichern = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_alle_rechte(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_rechte_speichern}'>
                </div>
                <div>
                    <p>Alle Rechte</p>
                    <p>speichern unter</p>
                </div>
            </label>
        </div> 
        ")
    };

    let alle_fehler_speichern = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_alle_fehler(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_fehler_speichern}'>
                </div>
                <div>
                    <p>Alle Fehler</p>
                    <p>speichern unter</p>
                </div>
            </label>
        </div> 
        ")
    };

    let alle_teibelast_speichern = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_alle_teilbelastungen(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_export_teilbelastungen}'>
                </div>
                <div>
                    <p>Alle Teilbelast.</p>
                    <p>speichern unter</p>
                </div>
            </label>
        </div> 
        ")
    };

    let alle_abt1_speichern = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_alle_abt1(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_export_abt1}'>
                </div>
                <div>
                    <p>Alle Abt. 1</p>
                    <p>speichern unter</p>
                </div>
            </label>
        </div> 
        ")
    };

    let alle_hvm_speichern = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_alle_hvm(event)'  class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_hvm}'>
                </div>
                <div>
                    <p>Alle HVM</p>
                    <p>speichern unter</p>
                </div>
            </label>
        </div> 
        ")
    };

    let export_pdf = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.open_export_pdf(event)' class='__application-ribbon-action-vertical-large'>
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

    static RELOAD_PNG: &[u8] = include_bytes!("../src/img/icons8-synchronize-48.png");
    let icon_reload = base64::encode(&RELOAD_PNG);

    let daten_importieren = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='reloadGrundbuch(event)' class='__application-ribbon-action-vertical-large'>
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

    let aenderungen_uebernehmen = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.upload_grundbuch(event)' class='__application-ribbon-action-vertical-large'>
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

    let ribbon_body = match rpc_data.tab.unwrap_or_default() {
        0 => {
            format!(
                "
            <div class='__application-ribbon-header'>
                <p onmouseup='selectTab(0);' class='active'>START</p>
                <p onmouseup='selectTab(1);'>KORREKTUR</p>
                <p onmouseup='selectTab(2);'>EXPORT</p>
            </div>
            <div class='__application-ribbon-body'>
                <div class='__application-ribbon-section 1'>
                    <div style='display:flex;flex-direction:row;'>
                        
                        {grundbuch_oeffnen}

                        {neues_grundbuch}
                        
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
                        {aenderungen_uebernehmen}

                        {export_pdf}
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
        1 => {
            format!(
                "
            <div class='__application-ribbon-header'>
                <p onmouseup='selectTab(0);'>START</p>
                <p onmouseup='selectTab(1);' class='active'>KORREKTUR</p>
                <p onmouseup='selectTab(2);'>EXPORT</p>
            </div>
            <div class='__application-ribbon-body'>
                <div class='__application-ribbon-section 4'>
                    <div style='display:flex;flex-direction:row;'>
                        
                        {alle_rechte_speichern}
                        
                        {alle_fehler_speichern}
                        
                        {alle_teibelast_speichern}

                        {alle_abt1_speichern}
                        
                        {alle_hvm_speichern}
                        
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
            </div>
            <div class='__application-ribbon-body'>


                {nebenbet}

                <div style='display:flex;flex-grow:1;'></div>

                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_lefis}
                    </div>
                </div>
            </div>
        "
            )
        }
    };

    normalize_for_js(ribbon_body)
}

pub fn render_main(_rpc_data: &UiData) -> String {
    let mb_token = include_str!("../MAPBOX_TOKEN.txt");
    let map = format!("
        <div id='__application-main-container' style='display:flex;flex-grow:1;'>
            <input type='hidden' id='mapboxtoken' style='display:none;' value='{mb_token}'></input>
            <div id='map' style='position:relative;width:100%;height:100%;'></div>
            <div id='__application_main-overlay-container' style='position:absolute;height:100%;width:100%;'>
                <div id='project' style='background:white;height:100%;'>
                    <h4>PROJEKT</h4>
                </div>
                <div id='__application-daten-laden'>
                    <button>XML Datei laden</button>
                </div>
            </div>
        </div>
    ");
    normalize_for_js(map) // TODO
}

pub fn normalize_for_js(s: String) -> String {
    s.lines()
        .map(|s| s.trim().replace('`', "'"))
        .collect::<Vec<_>>()
        .join("")
}