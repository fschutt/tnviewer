use core::f64;
use std::{collections::{BTreeMap, BTreeSet}, f64::MAX, vec};

use float_cmp::approx_eq;
use geo::{Centroid, TriangulateEarcut};
use quadtree_f32::{Item, QuadTree};
use serde_derive::{Serialize, Deserialize};
use web_sys::{console::log_1, js_sys::Atomics::xor};

use crate::{
    csv::{CsvDataType, Status}, geograf::points_to_rect, nas::{self, intersect_polys, point_is_in_polygon, translate_to_geo_poly, xor_polys, NasXMLFile, NasXmlQuadTree, SplitNasXml, SplitNasXmlQuadTree, SvgLine, SvgPoint, SvgPolygon, TaggedPolygon}, pdf::{join_polys, subtract_from_poly, FlurstueckeInPdfSpace, Konfiguration, ProjektInfo, Risse}, search::NutzungsArt, ui, uuid_wasm::{log_status, uuid}, xlsx::FlstIdParsed, xml::XmlNode
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UiData {
    #[serde(default)]
    pub popover_state: Option<PopoverState>,
    #[serde(default)]
    pub tab: Option<usize>,
    #[serde(default)]
    pub tool: Option<Tool>,
    #[serde(default)]
    pub selected_edit_flst: String,
    #[serde(default)]
    pub secondary_content: Option<bool>,
    #[serde(default)]
    pub render_out: Option<bool>,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Tool {
    #[serde(rename = "rissgebiet-zeichnen")]
    RissgebietZeichnen,
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

#[derive(Debug, Copy, PartialEq, Serialize, Deserialize, PartialOrd, Clone)]
pub enum ConfigurationView {
    #[serde(rename = "allgemein")]
    Allgemein,
    #[serde(rename = "darstellung-bearbeitung")]
    DarstellungBearbeitung,
    #[serde(rename = "darstellung-pdf")]
    DarstellungPdf,
    #[serde(rename = "darstellung-pdf-allgemein")]
    DarstellungPdfAllgemein,
    #[serde(rename = "pdf-beschriftungen")]
    PdfBeschriftungen,
    #[serde(rename = "pdf-symbole")]
    PdfSymbole,
}

#[derive(Debug, Copy, Serialize, Deserialize, PartialEq, PartialOrd, Clone)]
pub struct ContextMenuData {
    pub x: f32,
    pub y: f32,
}

// render entire <body> node depending on the state of the rpc_data
pub fn render_entire_screen(
    projekt_info: &ProjektInfo,
    risse: &Risse,
    rpc_data: &UiData, 
    csv: &CsvDataType, 
    aenderungen: &Aenderungen,
    konfiguration: &Konfiguration,
) -> String {
    normalize_for_js(format!(
        "
            <div id='__application_popover_search' style='
                pointer-events:none;
                width: 100%;
                height: 100%;
                min-height: 100%;
                position: fixed;
                z-index:999;
            '></div>
            {popover}
            <div id='__application-ribbon'>
                {ribbon_ui}
            </div>
            <div id='__application-main' style='overflow:hidden;'>
                {main}
            </div>
        ",
        popover = render_popover(rpc_data, konfiguration),
        ribbon_ui = render_ribbon(rpc_data, csv.is_empty()),
        main = render_main(projekt_info, risse, rpc_data, csv, aenderungen),
    ))
}

pub fn render_popover(rpc_data: &UiData, konfiguration: &Konfiguration) -> String {
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
        render_popover_content(rpc_data, konfiguration)
    );

    normalize_for_js(popover)
}

pub fn base64_encode<T: AsRef<[u8]>>(input: T) -> String {
    base64::encode(input)
}

pub fn ui_render_search_popover_content(term: &str) -> String {
    let mut pc = crate::search::search_map(term).into_iter().take(4).map(|(k, v)| {
        let bez = &v.bez;
        format!("
        <p style='padding: 0px 10px;font-size: 14px;color: #444;margin-top: 5px;'>{k} ({bez})</p>
        <div style='line-height:1.5;cursor:pointer;'>
            <div class='kontextmenü-eintrag' data-seite-neu='bv-horz'>
                {}
            </div>
        </div>", v.def + " " + v.ehb.as_str()
    )
    }).collect::<Vec<_>>().join("");

    

    if pc.is_empty() {
        pc = "<p style='padding: 0px 10px;font-size: 14px;color: #444;margin-top: 5px;'>Keine Suchergebnisse</p>".to_string();
    }

    let pc = format!("
        <div style='background:transparent;width: 100%;height: 100%;min-height: 100%;z-index:1001;pointer-events:all;' onmouseup='closePopOver()'>
            <div style='pointer-events: unset;padding: 1px;position: absolute;right: 5px;top: 30px;max-width: 302px;background: white;border-radius: 0px;'>
                <div style='border:1px solid #efefef;border-radius:5px;'>
                    {pc}
                </div>
            </div>
        </div>", 
    );

    normalize_for_js(pc)
}

pub fn render_popover_content(rpc_data: &UiData, konfiguration: &Konfiguration) -> String {
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

    let close_button = format!("f
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
            let active_darstellung_pdf = if *cw == DarstellungPdf { " active" } else { "" };
            let active_darstellung_bearbeitung = if *cw == DarstellungBearbeitung { " active" } else { "" };
            let active_darstellung_pdf_allgemein = if *cw == DarstellungPdfAllgemein { " active" } else { "" };
            let active_pdf_beschriftungen = if *cw == PdfBeschriftungen { " active" } else { "" };
            let active_pdf_symbole = if *cw == PdfSymbole { " active" } else { "" };

            let sidebar = format!("
                <div class='__application_configuration_sidebar' style='display:flex;flex-direction:column;width:160px;min-height:750px;'>
                    
                    <div class='__application_configuration_sidebar_section{active_allgemein}' onmouseup='activateConfigurationView(event, \"allgemein\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_settings}'></img>
                        <p>Allgemein</p>
                    </div>
                    
                    <hr/>
                    
                    <div class='__application_configuration_sidebar_section{active_darstellung_bearbeitung}' onmouseup='activateConfigurationView(event, \"darstellung-bearbeitung\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>Bearbeitung</p>
                    </div>

                    <div class='__application_configuration_sidebar_section{active_darstellung_pdf_allgemein}' onmouseup='activateConfigurationView(event, \"darstellung-pdf-allgemein\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>PDF Allgemein</p>
                    </div>

                    <div class='__application_configuration_sidebar_section{active_darstellung_pdf}' onmouseup='activateConfigurationView(event, \"darstellung-pdf\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>PDF Ebenen</p>
                    </div>

                    <div class='__application_configuration_sidebar_section{active_pdf_beschriftungen}' onmouseup='activateConfigurationView(event, \"pdf-beschriftungen\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>PDF Beschriftungen</p>
                    </div>

                    <div class='__application_configuration_sidebar_section{active_pdf_symbole}' onmouseup='activateConfigurationView(event, \"pdf-symbole\")'>
                        <img style='width:25px;height:25px;' src='data:image/png;base64,{img_clean}'></img>
                        <p>PDF Symbole</p>
                    </div>
                </div>
            ");

            let main_content = match cw {
                Allgemein => format!("
                    <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                        <div>
                            <h2 style='font-size:20px;'>Allgemein</h2>
                            
                            <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                <label style='font-size:20px;font-style:italic;'>Basiskarte</label>
                                <input type='text' class='konfiguration-editfield1' value='{basiskarte}' data-konfiguration-textfield='map-basiskarte' onchange='editKonfigurationTextField(event)'></input>
                            </div>
                    
                            <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                <label style='font-size:20px;font-style:italic;'>DOP Quelle</label>
                                <input type='text' class='konfiguration-editfield1' value='{dop_source}' data-konfiguration-textfield='map-dop-source' onchange='editKonfigurationTextField(event)'></input>
                            </div>

                            <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                <label style='font-size:20px;font-style:italic;'>DOP Ebene</label>
                                <input type='text' class='konfiguration-editfield1' value='{dop_layer}' data-konfiguration-textfield='map-dop-layer' onchange='editKonfigurationTextField(event)'></input>
                            </div>
                        </div>
                    </div>
                ",
                    basiskarte = konfiguration.map.basemap.clone().unwrap_or_default().trim(),
                    dop_source = konfiguration.map.dop_source.clone().unwrap_or_default().trim(),
                    dop_layer = konfiguration.map.dop_layers.clone().unwrap_or_default().trim(),
                ),
                DarstellungBearbeitung => {
                    format!("
                        <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                            <div>
                                <h2 style='font-size:20px;'>Bearbeitung</h2>
                                
                                <button onclick='konfigurationLayerNeu(event)' data-konfiguration-type='style' style='display: flex;width: 100%;margin-bottom: 10px;margin-top: 10px;cursor: pointer;background: #d1e9d7;padding: 10px;border-radius: 5px;'>Neue Ebene anlegen</button>
                                
                                <div style='max-height:500px;overflow-y:scroll'>
                                {edit_fields_bearbeitung}
                                </div>
                            </div>
                        </div>
                    ", edit_fields_bearbeitung = konfiguration.style.get_styles_sorted().iter().map(|(k, v)| {
                        format!("
                            <div style='padding:10px;margin-bottom:5px;background:#cccccc;'>
                                <div style='display: flex;flex-direction: row;justify-content: flex-end;'>
                                    <input type='text' class='konfiguration-editfield1' value='{name}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-style-name' onchange='editKonfigurationTextField(event)' style='flex-grow:1;'></input>
                                    <button onclick='moveOrDeleteKonfigurationField(event);' data-move-type='move-up' data-konfiguration-type='style' data-konfiguration-style-id='{k}' style='padding: 2px 5px;margin-left: 5px;cursor: pointer;border-radius: 3px;'>^ Layer anheben</button>
                                    <button onclick='moveOrDeleteKonfigurationField(event);' data-move-type='move-down'  data-konfiguration-type='style' data-konfiguration-style-id='{k}' style='padding: 2px 5px;margin-left: 5px;cursor: pointer;border-radius: 3px;'>v Layer absenken</button>
                                    <button onclick='moveOrDeleteKonfigurationField(event);' data-move-type='delete' data-konfiguration-type='style' data-konfiguration-style-id='{k}' style='padding: 2px 5px;margin-left: 5px;cursor: pointer;border-radius: 3px;'>Layer löschen</button>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Füllung</label>
                                    <input type='color' class='konfiguration-editfield1' value='{fill_color}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-style-fillcolor' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Umrandung</label>
                                    <input type='color' class='konfiguration-editfield1' value='{outline_color}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-style-outlinecolor' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Umrandung</label>
                                    <input type='number' class='konfiguration-editfield1' value='{outline_thickness}' minval='0' maxval='10' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-style-outlinethickness' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                            </div>
                            ", 
                                name = v.name.trim(),
                                fill_color = v.fill_color.clone().unwrap_or("#000000".to_string()),
                                outline_color = v.outline_color.clone().unwrap_or("#000000".to_string()),
                                outline_thickness = v.outline_thickness.clone().unwrap_or(0.0),
                            )
                        }).collect::<Vec<_>>().join("")
                    )
                },
                DarstellungPdf => {
                    format!("
                        <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                            <div>
                                <h2 style='font-size:20px;'>PDF Ebenen</h2>
                                
                                <!-- NORDPFEIL SVG ... -->

                                <button onclick='konfigurationLayerNeu(event)' data-konfiguration-type='pdf-nutzungsarten' style='display: flex;width: 100%;margin-bottom: 10px;margin-top: 10px;cursor: pointer;background: #d1e9d7;padding: 10px;border-radius: 5px;'>Neue Ebene anlegen</button>

                                <button onclick='konfigurationLayerAlle(event)' data-konfiguration-type='style' style='display: flex;width: 100%;margin-bottom: 10px;margin-top: 10px;cursor: pointer;background: #d1e9d7;padding: 10px;border-radius: 5px;'>Alle sichtbaren Kürzel übernehmen</button>

                                <div style='max-height:500px;overflow-y:scroll'>
                                {edit_fields_pdf}
                                </div>
                                
                            </div>
                        </div>
                    ", edit_fields_pdf = konfiguration.pdf.get_nutzungsarten_sorted().iter().map(|(k, v)| {
                        format!("
                            <div style='padding:10px;margin-bottom:5px;background:#cccccc;'>
                                <div style='display: flex;flex-direction: row;justify-content: flex-end;'>
                                    <input type='text' class='konfiguration-editfield1' value='{name}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-name' onchange='editKonfigurationTextField(event)' style='flex-grow:1;'></input>
                                    <button onclick='moveOrDeleteKonfigurationField(event);' data-move-type='move-up'  data-konfiguration-type='pdf-nutzungsarten' data-konfiguration-style-id='{k}' style='padding: 2px 5px;margin-left: 5px;cursor: pointer;border-radius: 3px;'>^ Layer anheben</button>
                                    <button onclick='moveOrDeleteKonfigurationField(event);' data-move-type='move-down'  data-konfiguration-type='pdf-nutzungsarten' data-konfiguration-style-id='{k}' style='padding: 2px 5px;margin-left: 5px;cursor: pointer;border-radius: 3px;'>v Layer absenken</button>
                                    <button onclick='moveOrDeleteKonfigurationField(event);' data-move-type='delete'  data-konfiguration-type='pdf-nutzungsarten' data-konfiguration-style-id='{k}' style='padding: 2px 5px;margin-left: 5px;cursor: pointer;border-radius: 3px;'>Layer löschen</button>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Füllung</label>
                                    <input type='color' class='konfiguration-editfield1' value='{fill_color}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-fillcolor' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Umrandung</label>
                                    <input type='color' class='konfiguration-editfield1' value='{outline_color}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-outlinecolor' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Umrandung</label>
                                    <input type='number' class='konfiguration-editfield1' value='{outline_thickness}' minval='0' maxval='10' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-outlinethickness' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Strichelung Umrandung</label>
                                    <input type='text' class='konfiguration-editfield1' value='{outline_dash}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-outlinedash' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Umrandung überdrucken (Overprint)</label>
                                    <input type='checkbox' {outline_overprint} class='konfiguration-editfield1' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-outline-overprint' onchange='editKonfigurationTextField(event)'></input>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Muster</label>
                                    <input id='__application_pdf_nutzungsart_pattern_{k}' type='hidden' style='display:none;' onchange='editKonfigurationTextField(event)' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-svg' data-konfiguration-style-id='{k}' value='{pattern_svg}'></input>
                                    <input type='file' accept='.svg' style='display:flex;' class='konfiguration-editfield1' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-svg' onchange='editKonfigurationInputFile(event)'></input>
                                </div>
                                
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Platzierung Muster</label>
                                    <select value='{pattern_placement}' style='display:flex;flex-grow:1;max-width:300px;' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-type' onchange='editKonfigurationTextField(event)'>
                                        {pattern_type_select_options}
                                    </select>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Beschriftung Schrift</label>
                                    <input type='text' class='konfiguration-editfield1' value='{beschriftung_schriftart}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-beschriftung-schriftart' onchange='editKonfigurationTextField(event)'></input>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Beschriftung Schriftgröße</label>
                                    <input type='number' class='konfiguration-editfield1' value='{beschriftung_schriftgroesse}' data-konfiguration-style-id='{k}' minval='0' maxval='50' data-konfiguration-textfield='map-pdf-nutzungsart-beschriftung-schriftgroesse' onchange='editKonfigurationTextField(event)'></input>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>Beschriftung Schrift</label>
                                    <input type='color' class='konfiguration-editfield1' value='{beschriftung_schriftfarbe}' data-konfiguration-style-id='{k}' data-konfiguration-textfield='map-pdf-nutzungsart-beschriftung-schriftfarbe' onchange='editKonfigurationTextField(event)'></input>
                                </div>
                            </div>
                            ", 
                                k = k,
                                name = v.kuerzel.clone(),
                                fill_color = v.fill_color.clone().unwrap_or("#000000".to_string()),
                                outline_color = v.outline_color.clone().unwrap_or("#000000".to_string()),
                                outline_thickness = v.outline_thickness.clone().unwrap_or(0.0),
                                outline_dash = v.outline_dash.clone().unwrap_or_default(),
                                outline_overprint = if v.outline_overprint { "checked='checked'" } else { "" },
                                pattern_svg = v.pattern_svg.clone().unwrap_or_default(),
                                pattern_placement = v.pattern_placement.clone().unwrap_or_default(),
                                pattern_type_select_options = {
                                    vec![
                                        ("none", "Kein Muster"),
                                        ("mitte", "mittig platzieren"),
                                        ("pattern", "Wiederholen (kein Versatz)"),
                                        ("pattern-alternate", "Mit Versatz wiederholen"),
                                    ].iter().map(|(k, v)| {
                                        format!("<option value='{k}'>{v}</option>")
                                    }).collect::<Vec<_>>().join("")
                                },
                                beschriftung_schriftart = v.lagebez_ohne_hsnr.font.clone().unwrap_or_default(),
                                beschriftung_schriftgroesse = v.lagebez_ohne_hsnr.fontsize.unwrap_or(0.0),
                                beschriftung_schriftfarbe = v.lagebez_ohne_hsnr.color.clone().unwrap_or("#000000".to_string()),
                            )
                        }).collect::<Vec<_>>().join("")
                    )
                },
                DarstellungPdfAllgemein => {
                    format!("
                        <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                            <div>
                                <h2 style='font-size:20px;'>PDF Allgemein</h2>
                                
                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>SVG Grenzpunkt</label>
                                    <input id='__application_pdf_nutzungsart_pattern_svg_grenzpunkt' type='hidden' style='display:none;' onchange='editKonfigurationTextField(event)' data-konfiguration-textfield='map-pdf-svg-grenzpunkt' data-konfiguration-style-id='svg_grenzpunkt' value='{grenzpunkt_svg}'></input>
                                    <input type='file' accept='.svg' style='display:flex;' class='konfiguration-editfield1' data-konfiguration-style-id='svg_grenzpunkt' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-svg' onchange='editKonfigurationInputFile(event)'></input>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>SVG Pfeil</label>
                                    <input id='__application_pdf_nutzungsart_pattern_svg_pfeil' type='hidden' style='display:none;' onchange='editKonfigurationTextField(event)' data-konfiguration-textfield='map-pdf-svg-pfeil' data-konfiguration-style-id='svg_pfeil' value='{pfeil_svg}'></input>
                                    <input type='file' accept='.svg' style='display:flex;' class='konfiguration-editfield1' data-konfiguration-style-id='svg_pfeil' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-svg' onchange='editKonfigurationInputFile(event)'></input>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>SVG Nordpfeil</label>
                                    <input id='__application_pdf_nutzungsart_pattern_svg_nordpfeil' type='hidden' style='display:none;' onchange='editKonfigurationTextField(event)' data-konfiguration-textfield='map-pdf-svg-nordpfeil' data-konfiguration-style-id='svg_nordpfeil' value='{nordpfeil_svg}'></input>
                                    <input type='file' accept='.svg' style='display:flex;' class='konfiguration-editfield1' data-konfiguration-style-id='svg_nordpfeil' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-svg' onchange='editKonfigurationInputFile(event)'></input>
                                </div>

                                <div style='display:flex;justify-content:space-between;padding:10px 0px;font-size:16px;'>
                                    <label style='font-size:12px;font-style:italic;'>SVG Gebäude löschen</label>
                                    <input id='__application_pdf_nutzungsart_pattern_svg_gebaeude_loeschen' type='hidden' style='display:none;' onchange='editKonfigurationTextField(event)' data-konfiguration-textfield='map-pdf-svg-gebaeude-loeschen' data-konfiguration-style-id='svg_gebaeude_loeschen' value='{gebaeude_loeschen_svg}'></input>
                                    <input type='file' accept='.svg' style='display:flex;' class='konfiguration-editfield1' data-konfiguration-style-id='svg_gebaeude_loeschen' data-konfiguration-textfield='map-pdf-nutzungsart-pattern-svg' onchange='editKonfigurationInputFile(event)'></input>
                                </div>
                            </div>
                        </div>
                    ",
                        grenzpunkt_svg = konfiguration.pdf.grenzpunkt_svg.clone().unwrap_or_default(),
                        pfeil_svg = konfiguration.pdf.pfeil_svg.clone().unwrap_or_default(),
                        nordpfeil_svg = konfiguration.pdf.nordpfeil_svg.clone().unwrap_or_default(),
                        gebaeude_loeschen_svg = konfiguration.pdf.gebauede_loeschen_svg.clone().unwrap_or_default(),
                    )
                
                },
                PdfBeschriftungen => {
                    format!("
                        <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                            <div>
                                <h2 style='font-size:20px;'>PDF Beschriftungen</h2>
                                
                            </div>
                        </div>
                    ",
                    )
                },
                PdfSymbole => {
                    format!("
                        <div style='padding:5px 0px;display:flex;flex-direction:column;flex-grow:1;'>
                            <div>
                                <h2 style='font-size:20px;'>PDF Symbole</h2>
                                
                            </div>
                        </div>
                    ",
                    )
                }
            };

            let main = format!("<div style='display:flex;flex-grow:1;padding:0px 20px;line-height: 1.2;'>{main_content}</div>");

            format!("
                <div style='box-shadow:0px 0px 100px #22222288;pointer-events:initial;width:1000px;position:relative;display:flex;flex-direction:column;margin:10px auto;border:1px solid grey;background:white;padding:100px;border-radius:5px;' onmousedown='event.stopPropagation();' onmouseup='event.stopPropagation();'>
                
                    {close_button}
                    
                    <h2 style='font-size:24px;margin-bottom:15px;font-family:sans-serif;'>Konfiguration</h2>

                    <div style='display:flex;flex-direction:row;flex-grow:1;width:100%;'>
                        {sidebar}
                        {main}
                    </div>
                </div>
            "
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

pub fn render_ribbon(rpc_data: &UiData, data_loaded: bool) -> String {
    
    static ICON_ZURUECK: &[u8] = include_bytes!("./img/icons8-back-48.png");
    let icon_back_base64 = base64_encode(ICON_ZURUECK);

    static ICON_VORWAERTS: &[u8] = include_bytes!("./img/icons8-forward-48.png");
    let icon_forward_base64 = base64_encode(ICON_VORWAERTS);

    static ICON_EINSTELLUNGEN: &[u8] = include_bytes!("./img/icons8-settings-48.png");
    let icon_settings_base64 = base64_encode(ICON_EINSTELLUNGEN);

    static ICON_HELP: &[u8] = include_bytes!("./img/icons8-help-96.png");
    let icon_help_base64 = base64_encode(ICON_HELP);

    static ICON_INFO: &[u8] = include_bytes!("./img/icons8-info-48.png");
    let icon_info_base64 = base64_encode(ICON_INFO);

    static ICON_GRUNDBUCH_OEFFNEN: &[u8] = include_bytes!("./img/icons8-book-96.png");
    let icon_open_base64 = base64_encode(ICON_GRUNDBUCH_OEFFNEN);

    static ICON_SPEICHERN: &[u8] = include_bytes!("./img/icons8-save-96.png");
    let icon_save_base64 = base64_encode(ICON_SPEICHERN);

    static ICON_TRANSFER: &[u8] = include_bytes!("./img/icons8-transfer-96.png");
    let icon_transfer_base64 = base64_encode(ICON_TRANSFER);

    static ICON_XML: &[u8] = include_bytes!("./img/icons8-xml-96.png");
    let icon_xml_base64 = base64_encode(ICON_XML);

    static ICON_HOUSE: &[u8] = include_bytes!("./img/icons8-house-96.png");
    let icon_house_base64 = base64_encode(ICON_HOUSE);

    static ICON_DAVID: &[u8] = include_bytes!("./img/icons8-david-96.png");
    let icon_david_base64 = base64_encode(ICON_DAVID);

    static ICON_GEOGRAF: &[u8] = include_bytes!("./img/geograf-96.png");
    let icon_geograf_base64 = base64_encode(ICON_GEOGRAF);

    static ICON_GEORG: &[u8] = include_bytes!("./img/georg-96.png");
    let icon_georg_base64 = base64_encode(ICON_GEORG);

    static ICON_NEU: &[u8] = include_bytes!("./img/icons8-add-file-96.png");
    let icon_neu_base64 = base64_encode(ICON_NEU);

    static ICON_EXCEL: &[u8] = include_bytes!("./img/icons8-microsoft-excel-2019-96.png");
    let icon_export_csv = base64_encode(ICON_EXCEL);

    static ICON_BROOM: &[u8] = include_bytes!("./img/icons8-broom-96.png");
    let icon_export_lefis = base64_encode(ICON_BROOM);

    static ICON_LOG: &[u8] = include_bytes!("./img/icons8-log-96.png");
    let icon_log_base64 = base64_encode(ICON_LOG);

    // TAB 1
    // let disabled = if data_loaded { "" } else { "disabled" };
    let disabled = "";
    
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
                    <img class='icon' src='data:image/png;base64,{icon_xml_base64}'>
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
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_save_base64}'>
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
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_house_base64}'>
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
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_transfer_base64}'>
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
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.export_excel(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_csv}'>
                    </div>
                    <div>
                        <p>Export</p>
                        <p>Excel</p>
                    </div>
                </label>
            </div>
        ")
    };

    let export_eigentuemer = {
        format!("
            <div class='__application-ribbon-section-content'>
                <label onmouseup='tab_functions.export_flst_nach_eigentuemer(event)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_csv}'>
                    </div>
                    <div>
                        <p>Export Flst.</p>
                        <p>n. Eigentümer</p>
                    </div>
                </label>
            </div>
        ")
    };

    let export_geograf = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='tab_functions.export_geograf(event)' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_geograf_base64}'>
                </div>
                <div>
                    <p>Export</p>
                    <p>GEOgraf</p>
                </div>
            </label>
        </div>   
        ")
    };

    let export_log = {
        format!("
        <div class='__application-ribbon-section-content'>
            <label onmouseup='exportLog();' class='__application-ribbon-action-vertical-large'>
                <div class='icon-wrapper'>
                    <img class='icon {disabled}' src='data:image/png;base64,{icon_log_base64}'>
                </div>
                <div>
                    <p>Log-Datei</p>
                    <p>speichern</p>
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
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_georg_base64}'>
                    </div>
                    <div>
                        <p>Export Flst.</p>
                        <p>nach Georg</p>
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
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_david_base64}'>
                    </div>
                    <div>
                        <p>Export</p>
                        <p>DAVID</p>
                    </div>
                </label>
            </div>
        ")
    };

    let clean_stage7_test = {
        format!("

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(1);' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>1: Punkte auf</p>
                        <p>Änd. ziehen</p>
                    </div>
                </label>
            </div>

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(2);' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>2: Punkte einf.</p>
                        <p>von nahen Änder.</p>
                    </div>
                </label>
            </div>

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(25);' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>2.5: Änderungen</p>
                        <p>mergen nach Typ</p>
                    </div>
                </label>
            </div>

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(3);' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>3: Punkte auf</p>
                        <p>Flst. ziehen</p>
                    </div>
                </label>
            </div>

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(4);' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>4: Punkte einf.</p>
                        <p>von nahen Flst.</p>
                    </div>
                </label>
            </div>

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(5);' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>5: Überlappende</p>
                        <p>Änd. subtrahieren</p>
                    </div>
                </label>
            </div>

            <div class='__application-ribbon-section-content'>
                <label onmouseup='cleanStage(7)' class='__application-ribbon-action-vertical-large'>
                    <div class='icon-wrapper'>
                        <img class='icon {disabled}' src='data:image/png;base64,{icon_export_lefis}'>
                    </div>
                    <div>
                        <p>Änderungen</p>
                        <p>zu Splitfl.</p>
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

                <div class='__application-ribbon-section 2'>
                    <div style='display:flex;flex-direction:row;'>
                        {gebaeude_loeschen}
                        {nutzung_einzeichnen}
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
        _ => {
            format!(
                "
            <div class='__application-ribbon-header'>
                <p onmouseup='selectTab(0);'>START</p>
                <p onmouseup='selectTab(2);' class='active'>EXPORT</p>
                <div style='flex-grow:1;'></div>
                <p id='export-status'></p>
                <input type='search' placeholder='Nutzungsarten durchsuchen...' style='margin-right:5px;margin-top:5px;min-width:300px;border:1px solid gray;max-height:25px;padding:5px;' oninput='searchNA(event);' onchange='searchNA(event);' onfocusout='closePopOver();'></input>
            </div>
            <div class='__application-ribbon-body'>

                <div class='__application-ribbon-section 2'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_excel}
                        {export_eigentuemer}
                    </div>
                </div>

                <div class='__application-ribbon-section 2'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_alle_flurstuecke}
                        {export_geograf}
                        {export_david}
                    </div>
                </div>

                <div class='__application-ribbon-section 2'>
                    <div style='display:flex;flex-direction:row;'>
                        {export_log}
                        {projekt_speichern}
                    </div>
                </div>

                <div class='__application-ribbon-section 5'>
                    <div style='display:flex;flex-direction:row;'>
                        {clean_stage7_test}
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

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct GebaeudeLoeschen {
    pub gebaeude_id: String,
    pub flst_id: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Aenderungen {
    pub gebaeude_loeschen: BTreeMap<String, GebaeudeLoeschen>,
    pub na_definiert: BTreeMap<FlstPartId, Kuerzel>,
    pub na_polygone_neu: BTreeMap<NewPolyId, PolyNeu>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AenderungenClean {
    pub nas_xml_quadtree: SplitNasXmlQuadTree,
    pub aenderungen: Aenderungen, 
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct AenderungenIntersections(pub Vec<AenderungenIntersection>);

impl AenderungenIntersections {

    pub fn get_future_flaechen(&self) -> Self {
        let mut vi = Vec::new();
        for s in self.0.iter() {
            vi.push(AenderungenIntersection {
                poly_cut: s.poly_cut.clone(),
                flst_id: s.flst_id.clone(),
                flst_id_part: s.flst_id_part.clone(),
                alt: s.neu.clone(),
                neu: s.neu.clone(),
            });
        }
        Self(vi).merge_to_nearest(true)
    }

    pub fn deduplicate(&self) -> Self {
        let mut aenderungen_2 = BTreeMap::new();

        for a in self.0.iter() {
            aenderungen_2.insert(a.poly_cut.get_hash(), a.clone());
        }

        Self(aenderungen_2.values().cloned().collect())
    }
    
    pub fn merge_to_nearest(&self, special: bool) -> Self {

        log_status(&format!("merge_to_nearest {special:?}"));

        let mut splitflaechen_by_flst_kuerzel = BTreeMap::new();

        let aenderungen_uuid = self.0
        .iter()
        .map(|sf| (uuid(), sf))
        .collect::<Vec<_>>();

        let aenderungen = if special {
            Aenderungen {
                gebaeude_loeschen: BTreeMap::new(),
                na_definiert: BTreeMap::new(),
                na_polygone_neu: aenderungen_uuid.iter().map(|(id, sf)| {
                    (id.clone(), PolyNeu {
                        nutzung: Some(sf.neu.clone()),
                        poly: sf.poly_cut.clone(),
                    })
                }).collect()
            }
            .clean_stage1(&mut Vec::new(), 1.0, 1.0)
        } else {
            Aenderungen::default()
        };

        log_status(&format!("merge_to_nearest 2"));

        for (id, s) in aenderungen_uuid.iter() {
            let s_poly = aenderungen.na_polygone_neu.get(id).map(|pn| &pn.poly).unwrap_or(&s.poly_cut);
            splitflaechen_by_flst_kuerzel.entry((s.flst_id.clone(), s.flst_id_part.clone()))
            .or_insert_with(|| BTreeMap::new())
            .entry((s.alt.clone(), s.neu.clone()))
            .or_insert_with(|| Vec::new())
            .push(s_poly.clone());
        }

        log_status(&format!("merge_to_nearest 3"));

        for (k0, v) in splitflaechen_by_flst_kuerzel.iter_mut() {
            let flst_id = FlstIdParsed::from_str(&k0.0).to_nice_string();
            log_status(&format!("{flst_id}: {}", serde_json::to_string(&v.iter().map(|(k, w)| (&k.0, &k.1, w.len())).collect::<Vec<_>>()).unwrap_or_default()));
            for (k1, k) in v.iter_mut() {

                let k2 = k.iter().flat_map(|p| p.outer_rings.iter().map(|l| {
                    let mut q = SvgPolygon::from_line(l); // TODO
                    if !p.inner_rings.is_empty() {
                        log_status("WARN!!!");
                    }
                    (q.get_hash(), q)
                })).collect::<BTreeMap<_, _>>();

                *k = k2.values().map(|s| (*s).clone()).collect::<Vec<_>>();

                if k.len() < 2 {
                    continue;
                }

                let polys_to_join = k.iter().cloned().collect::<Vec<_>>();

                let polys_to_join_len = polys_to_join.len();

                log_status(&format!("{flst_id} ({k1:?}): joining"));
                log_status(&serde_json::to_string(&polys_to_join).unwrap_or_default());
                let joined = match join_polys(&polys_to_join, false, false) {
                    Some(s) => s.recombine_polys(),
                    None => continue,
                };
                log_status(&serde_json::to_string(&joined).unwrap_or_default());
                log_status(&format!("{flst_id}: joined -----"));

                let joined_len = joined.len();

                log_status(&format!("{flst_id}: verbinde {polys_to_join_len} Flächen {k1:?} zu {joined_len} Flächen"));

                *k = joined;
            }
        }

        log_status(&format!("merge_to_nearest 4"));

        let new_sf = splitflaechen_by_flst_kuerzel.iter().flat_map(|((flst_id, flst_id_part), v)| {
            v.iter().flat_map(|((alt, neu), polys)| {
                polys.iter().map(|p| AenderungenIntersection {
                    alt: alt.clone(),
                    neu: neu.clone(),
                    flst_id: flst_id.clone(),
                    flst_id_part: flst_id_part.clone(),
                    poly_cut: p.clone(),
                })
            })
        }).collect();

        log_status(&format!("merge_to_nearest 5"));

        Self(new_sf)
    }

    pub fn clean_zero_size_areas(&self) -> Self {
        Self(
            self.0.iter()
            .filter(|q| !q.poly_cut.is_zero_area())
            .map(|q| {

                let filter = |rings: &[SvgLine]| {
                    rings.iter()
                    .filter(|s| {
                        !SvgPolygon::from_line(s).is_zero_area()
                    })
                    .filter(|s| {
                        !SvgPolygon::from_line(s).is_zero_area()
                    })
                    .map(|w| w.clone())
                    .collect::<Vec<_>>()
                };

                let mut q = q.clone();
                q.poly_cut.outer_rings = filter(&q.poly_cut.outer_rings);
                q.poly_cut.inner_rings = filter(&q.poly_cut.inner_rings);
                q
            })
            .filter(|q| !q.poly_cut.is_zero_area())
            .collect::<Vec<_>>()
        )
    }
    
    pub fn get_texte(s: &[AenderungenIntersection]) -> Vec<TextPlacement> {
        s.iter().flat_map(|q| {
            if q.alt == q.neu {
                q.poly_cut.outer_rings.iter().flat_map(|or| {
                    let p = SvgPolygon { outer_rings: vec![or.clone()], inner_rings: q.poly_cut.inner_rings.clone() };
                    let lp = match p.get_label_pos() {
                        Some(s) => s,
                        None => return Vec::new(),
                    };
                    vec![TextPlacement {
                        kuerzel: q.alt.clone(),
                        status: TextStatus::StaysAsIs,
                        pos: lp.clone(),
                        ref_pos: lp.clone(),
                        area: p.area_m2().round() as usize,
                        poly: p.clone(),
                    }]
                }).collect::<Vec<_>>()
            } else {
                let polys = q.poly_cut.outer_rings.iter().map(|or| {
                    SvgPolygon { outer_rings: vec![or.clone()], inner_rings: q.poly_cut.inner_rings.clone() }
                }).collect::<Vec<_>>();
                polys.iter().flat_map(|p| {
                    let lp = match p.get_label_pos() {
                        Some(s) => s,
                        None => return Vec::new(),
                    };
                    let sp = match p.get_secondary_label_pos() {
                        Some(s) => s,
                        None => lp,
                    };
                    vec![
                        TextPlacement {
                            kuerzel: q.alt.clone(),
                            status: TextStatus::Old,
                            pos: lp.clone(),
                            ref_pos: lp.clone(),
                            area: p.area_m2().round() as usize,
                            poly: p.clone(),
                        },
                        TextPlacement {
                            kuerzel: q.neu.clone(),
                            status: TextStatus::New,
                            pos: sp.clone(),
                            ref_pos: sp.clone(),
                            area: p.area_m2().round() as usize,
                            poly: p.clone(),
                        }
                    ]
                }).collect()
            }
        }).collect()
    }
}

impl AenderungenClean {
    pub fn get_aenderungen_intersections(&self, nas_xml: &NasXMLFile, gemarkung: usize) -> AenderungenIntersections {
        
        let mut is = Vec::new();
        
        let aenderung_len = self.aenderungen.na_polygone_neu.len();
        let mut flst_parts_changed = BTreeMap::new();
        let mut intersection_sizes = BTreeMap::new();

        for (id, (aenderung_i, polyneu)) in self.aenderungen.na_polygone_neu.iter().enumerate() {
            
            let neu_kuerzel = match polyneu.nutzung.clone() {
                Some(s) => s,
                None => continue,
            };

            let all_touching_flst_parts = self.nas_xml_quadtree.get_overlapping_flst(&polyneu.poly.get_rect());
            log_status(&format!("[{} / {aenderung_len}] Verschneide Änderung {aenderung_i} mit {} überlappenden Flächen", id + 1, all_touching_flst_parts.len()));

            for (potentially_touching_id, potentially_intersecting) in all_touching_flst_parts {
                
                let flurstueck_id = match potentially_intersecting.attributes.get("AX_Flurstueck") {
                    Some(s) => s.clone(),
                    None => continue,
                };

                let intersect_id = potentially_intersecting.attributes
                .get("AX_IntersectionId")
                .map(|w| format!(":{w}"))
                .unwrap_or_default();

                let ebene = match potentially_intersecting.attributes.get("AX_Ebene") {
                    Some(s) => s.clone(),
                    None => continue,
                };
                let obj_id = match potentially_intersecting.attributes.get("id") {
                    Some(s) => s.clone(),
                    None => continue,
                };
                let alt_kuerzel = match potentially_intersecting.get_auto_kuerzel(&ebene) {
                    Some(s) => s,
                    None => continue,
                };

                let anew = potentially_intersecting.poly.round_to_3dec();
                let bnew = polyneu.poly.round_to_3dec();

                let is_polys = intersect_polys(&anew, &bnew);
                let mut is_size = 0.0;
                let flst_id_part = format!("{potentially_touching_id}:{ebene}:{obj_id}{intersect_id}");
                for intersect_poly in is_polys {
                    let intersect_poly = intersect_poly.round_to_3dec();
                    if intersect_poly.is_zero_area() {
                        continue;
                    }
                    is_size += intersect_poly.area_m2();
                    flst_parts_changed.entry(flst_id_part.clone())
                    .or_insert_with(|| (TaggedPolygon {
                        attributes: potentially_intersecting.attributes.clone(),
                        poly: anew.clone(),
                    }, BTreeMap::new()))
                    .1.insert(aenderung_i.clone(), bnew.clone());

                    let qq = AenderungenIntersection {
                        alt: alt_kuerzel.clone(),
                        neu: neu_kuerzel.clone(),
                        flst_id: flurstueck_id.clone(),
                        flst_id_part: flst_id_part.clone(),
                        poly_cut: intersect_poly.round_to_3dec(),
                    };
                    log_status(&format!("Splitflächen (Stufe 1): {flst_id_part}: {alt_kuerzel} -> {neu_kuerzel} = {} m2", intersect_poly.area_m2().round()));
                    is.push(qq);
                }
            
                intersection_sizes.insert((flst_id_part, aenderung_i.clone()), is_size);
            }
        }

        let mut is = AenderungenIntersections(is)
        .clean_zero_size_areas()
        .deduplicate()
        .merge_to_nearest(false).0;

        log_status(&format!("OK: {} Flurstückteile verändert", flst_parts_changed.len()));

        for (flst_part_id, (flst_part, areas_to_subtract)) in flst_parts_changed {
            
            let ebene = match flst_part.attributes.get("AX_Ebene") {
                Some(s) => s.clone(),
                None => continue,
            };

            let alt_kuerzel = match flst_part.get_auto_kuerzel(&ebene) {
                Some(s) => s.clone(),
                None => continue,
            };

            let flurstueck_id = match flst_part.attributes.get("AX_Flurstueck") {
                Some(s) => s.clone(),
                None => continue,
            };

            let jp = join_polys(&areas_to_subtract.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>(), false, false);
            let areas_to_subtract_joined = match jp {
                Some(s) => s,
                None => continue,
            };

            let orig_size = flst_part.poly.area_m2();
            let areas_to_subtract_ids = areas_to_subtract.iter().map(|(id, _)| id.clone()).collect::<BTreeSet<_>>();
            let size_of_all_intersections = areas_to_subtract_ids.iter().map(|s| intersection_sizes.get(&(flst_part_id.clone(), s.clone())).copied().unwrap_or(0.0)).sum::<f64>();
            if orig_size - size_of_all_intersections < 1.0 {
                continue;
            }

            let subtracted = subtract_from_poly(&flst_part.poly, &[&areas_to_subtract_joined]);

            let neu_kuerzel = self.aenderungen.na_definiert
            .iter()
            .find_map(|(k, v)| if flst_part_id.starts_with(k) { Some(v) } else { None })
            .unwrap_or(&alt_kuerzel).clone();

            let qq = AenderungenIntersection {
                alt: alt_kuerzel.clone(),
                neu: neu_kuerzel.clone(),
                flst_id: flurstueck_id.clone(),
                flst_id_part: flst_part_id.clone(),
                poly_cut: subtracted.round_to_3dec(),
            };
            
            log_status(&format!("Splitflächen (Stufe 2): {flst_part_id}: {alt_kuerzel} -> {neu_kuerzel} = {} m2", subtracted.area_m2().round()));

            is.push(qq);
        }

        let mut is = is.into_iter()
        .filter(|i| !i.poly_cut.is_zero_area())
        .collect::<Vec<_>>();

        // insert na_definiert
        let na_bereits_definiert = is.iter()
        .map(|s| s.flst_id_part.clone())
        .collect::<BTreeSet<_>>();

        for (na_def, neu_kuerzel) in self.aenderungen.na_definiert.iter() {
            
            if na_bereits_definiert.iter().any(|s| na_def.starts_with(s) || s.starts_with(na_def)) {
                continue;
            }

            let flst_part = match self.nas_xml_quadtree.original.get_flst_part_by_id(&na_def) {
                Some(s) => s,
                None => continue,
            };

            let kuerzel = match flst_part.attributes.get("AX_Ebene").and_then(|k| flst_part.get_auto_kuerzel(k)) {
                Some(s) => s,
                None => continue,
            };

            let flst_id = match flst_part.attributes.get("AX_Flurstueck") {
                Some(s) => s.clone(),
                None => continue,
            };

            is.push(AenderungenIntersection {
                alt: kuerzel.clone(),
                neu: neu_kuerzel.clone(),
                flst_id: flst_id,
                flst_id_part: na_def.clone(),
                poly_cut: flst_part.poly.clone(),
            });

            log_status(&format!("Splitflächen (Stufe 3): {na_def}: {kuerzel} -> {neu_kuerzel} = {} m2", flst_part.poly.area_m2().round()));
        }

        let na_bereits_definiert = is.iter()
        .map(|s| s.flst_id_part.clone())
        .collect::<BTreeSet<_>>();

        // insert gebaeude geänderte flst
        for geb in self.aenderungen.gebaeude_loeschen.values() {
            for flst_id in geb.flst_id.iter() {
                let default = Vec::new();
                for part in self.nas_xml_quadtree.original.flurstuecke_nutzungen.get(flst_id).unwrap_or(&default) {
                    let flurstueck_id = match part.attributes.get("AX_Flurstueck") {
                        Some(s) => s.clone(),
                        None => continue,
                    };
                    let ebene = match part.attributes.get("AX_Ebene") {
                        Some(s) => s.clone(),
                        None => continue,
                    };
                    let obj_id = match part.attributes.get("id") {
                        Some(s) => s.clone(),
                        None => continue,
                    };

                    let intersect_id = part.attributes
                    .get("AX_IntersectionId")
                    .map(|w| format!(":{w}"))
                    .unwrap_or_default();

                    let flst_part_id = format!("{flurstueck_id}:{ebene}:{obj_id}{intersect_id}");
                    if na_bereits_definiert.iter().any(|s| flst_part_id.starts_with(s)) {
                        continue;
                    }
                    let kuerzel = match part.get_auto_kuerzel(&ebene) {
                        Some(s) => s,
                        None => continue,
                    };
                    is.push(AenderungenIntersection {
                        alt: kuerzel.clone(),
                        neu: kuerzel.clone(),
                        flst_id: flurstueck_id,
                        flst_id_part: flst_part_id.clone(),
                        poly_cut: part.poly.clone(),
                    });
                    log_status(&format!("Splitflächen (Stufe 4): {flst_part_id}: {kuerzel} -> {kuerzel} = {} m2", part.poly.area_m2().round()));
                }
            }
        }

        let mut is = is.into_iter()
        .filter(|i| !i.poly_cut.is_zero_area())
        .collect::<Vec<_>>();

        // let is = merge_adjacent_intersections(is);

        let default = Vec::new();

        // Remove splitflächen für flurstücke die keine Änderung haben
        let flst_touched = is.iter().map(|s| s.flst_id.clone()).collect::<BTreeSet<_>>();
        let to_remove_flst = flst_touched.iter().filter_map(|flst_id| {
            if is.iter().filter(|s| s.flst_id == *flst_id).all(|s| s.alt == s.neu) {
                Some(flst_id)
            } else {
                None
            }
        }).collect::<BTreeSet<_>>();
        if !to_remove_flst.is_empty() {
            is.retain(|s| !to_remove_flst.contains(&s.flst_id));
        }

        let na_bereits_definiert = is.iter()
        .map(|s| s.flst_id_part.clone())
        .collect::<BTreeSet<_>>();

        // insert other flst areas (bleibt)
        let flst_touched = is.iter().map(|s| s.flst_id.clone()).collect::<BTreeSet<_>>();
        for flst_id in flst_touched.iter() {
            for part in self.nas_xml_quadtree.original.flurstuecke_nutzungen.get(flst_id).unwrap_or(&default) {
                let flurstueck_id = match part.attributes.get("AX_Flurstueck") {
                    Some(s) => s.clone(),
                    None => continue,
                };
                let ebene = match part.attributes.get("AX_Ebene") {
                    Some(s) => s.clone(),
                    None => continue,
                };
                let obj_id = match part.attributes.get("id") {
                    Some(s) => s.clone(),
                    None => continue,
                };
                let intersect_id = part.attributes
                .get("AX_IntersectionId")
                .map(|w| format!(":{w}"))
                .unwrap_or_default();
                let flst_part_id = format!("{flurstueck_id}:{ebene}:{obj_id}{intersect_id}");
                if na_bereits_definiert.iter().any(|s| flst_part_id.starts_with(s)) {
                    continue;
                }
                let kuerzel = match part.get_auto_kuerzel(&ebene) {
                    Some(s) => s,
                    None => continue,
                };
                is.push(AenderungenIntersection {
                    alt: kuerzel.clone(),
                    neu: kuerzel.clone(),
                    flst_id: flurstueck_id.clone(),
                    flst_id_part: flst_part_id.clone(),
                    poly_cut: part.poly.clone(),
                });
                log_status(&format!("Splitflächen (Stufe 5): {flst_part_id}: {kuerzel} -> {kuerzel} = {} m2", part.poly.area_m2().round()));
            }
        }

        let mut is = AenderungenIntersections(is)
        .clean_zero_size_areas()
        .deduplicate().0;

        // Remove Änderungen, wo nichts am Flurstück geändert wurde
        let alle_flst = is.iter().map(|s| s.flst_id.clone()).collect::<BTreeSet<_>>();
        let to_remove = alle_flst.iter().filter_map(|flst_id| {
            let all_splitflaechen_fuer_flst = is.iter().filter_map(|s| if s.flst_id == *flst_id { Some(s) } else { None }).collect::<Vec<_>>();
            if all_splitflaechen_fuer_flst.iter().all(|s| s.alt == s.neu) {
                Some(flst_id)
            } else {
                None
            }
        }).collect::<BTreeSet<_>>();

        is.retain(|s| !to_remove.contains(&s.flst_id));

        for a in to_remove.iter() {
            log_status(&format!("WARN: Lösche Änderung an Flst. {}: keine Änderungen", FlstIdParsed::from_str(&a).to_nice_string()));
        }

        is.retain(|s: &AenderungenIntersection| {
            FlstIdParsed::from_str(&s.flst_id).gemarkung == gemarkung.to_string()
        });

        let alle_flst = is.iter().map(|s| s.flst_id.clone()).collect::<BTreeSet<_>>();
        let _ = alle_flst.into_iter().filter_map(|flst_id| {
            let soll = self.nas_xml_quadtree.original.flurstuecke_nutzungen.get(&flst_id).unwrap_or(&default).iter().map(|tp| tp.poly.area_m2()).sum::<f64>().round();
            let ist = is.iter().filter_map(|s| if s.flst_id == flst_id { Some(s.poly_cut.area_m2()) } else { None }).sum::<f64>().round();
            if ist + 2.0 < soll {
                log_status(&format!("WARN: Loch in Flst {}: soll = {soll}, ist = {ist}", FlstIdParsed::from_str(&flst_id).to_nice_string()));
                Some(flst_id)
            } else if ist > soll + 2.0 {
                log_status(&format!("WARN: Doppelte Fläche in Flst {}: soll = {soll}, ist = {ist}", FlstIdParsed::from_str(&flst_id).to_nice_string()));
                Some(flst_id)
            } else {
                None
            }
        }).collect::<BTreeSet<_>>();

        AenderungenIntersections(is)
        .merge_to_nearest(false)
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct AenderungenIntersection {
    pub alt: Kuerzel,
    pub neu: Kuerzel,
    pub flst_id: FlstId,
    pub flst_id_part: String,
    pub poly_cut: SvgPolygon,
}

impl AenderungenIntersection {
    
    pub fn format_flst_id_func(s: &str) -> String {
        let s = FlstIdParsed::from_str(&s);
        let q = match s.parse_num() {
            Some(o) => o,
            None => return s.to_nice_string(),
        };
        q.format_nice()
    }

    pub fn format_flst_id(&self) -> String {
        Self::format_flst_id_func(&self.flst_id)
    }

    pub fn format_flst_id_search(&self) -> String {
        let s = FlstIdParsed::from_str(&self.flst_id);
        let q = match s.parse_num() {
            Some(o) => o,
            None => return s.to_nice_string(),
        };
        q.format_start_str()
    }

    pub fn get_splitflaechen_fuer_flst(splitflaechen: &[Self], flst_id: &str) -> Vec<AenderungenIntersection> {

        let flst_id = FlstIdParsed::from_str(flst_id);
        let flst_id = match flst_id.parse_num() {
            Some(o) => o.format_nice(),
            None => flst_id.to_nice_string(),
        };

        let mut splitflaechen_fuer_flst = splitflaechen.iter().filter(|s| {
            s.format_flst_id() == flst_id 
        }).cloned().collect::<Vec<_>>();
        splitflaechen_fuer_flst.sort_by(|a, b| a.alt.cmp(&b.alt));
        splitflaechen_fuer_flst.dedup();
        splitflaechen_fuer_flst
    }

    pub fn get_splitflaechen_fuer_flst_veraendert(splitflaechen: &[Self], flst_id: &str) -> Vec<AenderungenIntersection> {
        Self::get_splitflaechen_fuer_flst(splitflaechen, flst_id)
        .into_iter()
        .filter(|s| s.alt.as_str() != s.neu.as_str())
        .collect()
    }

    pub fn get_splitflaechen_fuer_flst_bleibt(splitflaechen: &[Self], flst_id: &str) -> Vec<AenderungenIntersection> {
        Self::get_splitflaechen_fuer_flst(splitflaechen, flst_id)
        .into_iter()
        .filter(|s| s.alt.as_str() == s.neu.as_str())
        .collect()
    }

    pub fn get_auto_notiz(splitflaechen: &[Self], flst_id: &str) -> String {

        let (alt_weg, neu_dazu, veraendert) = Self::get_auto_kuerzel(splitflaechen, flst_id);

        let mut neu_dazu_str = neu_dazu.into_iter().collect::<Vec<_>>().join(" / ");
        if !neu_dazu_str.trim().is_empty() {
            neu_dazu_str.push_str(" einzeichnen");
        }

        let mut alt_weg_str = alt_weg.into_iter().collect::<Vec<_>>().join(" / ");
        if !alt_weg_str.trim().is_empty() {
            alt_weg_str.push_str(" entfernen");
        }

        let mut veraendert_str = veraendert.into_iter().collect::<Vec<_>>().join(" / ");
        if !veraendert_str.trim().is_empty() {
            veraendert_str.push_str(" anpassen");
        }

        let mut v = Vec::new();
        
        if !alt_weg_str.trim().is_empty() {
            v.push(alt_weg_str);
        }
        if !neu_dazu_str.trim().is_empty() {
            v.push(neu_dazu_str);
        }
        if !veraendert_str.trim().is_empty() {
            v.push(veraendert_str);
        }

        v.join(", ").trim().to_string()
    }

    fn get_auto_kuerzel(splitflaechen: &[Self], flst_id: &str) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {

        let veraenderte_splitflaechen = Self::get_splitflaechen_fuer_flst_veraendert(splitflaechen, flst_id);
        
        if veraenderte_splitflaechen.is_empty() {
            return (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        }
    
        let bleibende_kuerzel = Self::get_splitflaechen_fuer_flst_bleibt(splitflaechen, flst_id)
            .iter().map(|s| s.alt.clone()).collect::<BTreeSet<_>>();

        let alt_join = veraenderte_splitflaechen.iter().map(|s| s.alt.clone()).collect::<BTreeSet<_>>();
        let neu_join = veraenderte_splitflaechen.iter().map(|s| s.neu.clone()).collect::<BTreeSet<_>>();
        
        let alt_weg = alt_join.difference(&neu_join).cloned().collect::<BTreeSet<_>>();
        let neu_dazu = neu_join.difference(&alt_join).cloned().collect::<BTreeSet<_>>();
        let mut veraendert = alt_join.intersection(&neu_join).cloned().collect::<BTreeSet<_>>();
        for k in bleibende_kuerzel.iter() {
            if alt_weg.contains(k) || neu_dazu.contains(k) {
                veraendert.insert(k.clone());
            }
        }
        let alt_weg = alt_weg.into_iter().filter(|s| !bleibende_kuerzel.contains(s)).collect::<BTreeSet<_>>();
        let neu_dazu = neu_dazu.into_iter().filter(|s| !bleibende_kuerzel.contains(s)).collect::<BTreeSet<_>>();

        (alt_weg, neu_dazu, veraendert)
    }

    pub fn get_auto_status(splitflaechen: &[Self], flst_id: &str) -> Status {

        let such_id = match FlstIdParsed::from_str(&flst_id).parse_num() {
            Some(s) => s,
            None => return Status::Bleibt,
        };

        let splitflaechen = splitflaechen.iter().filter(|sf| {
            let flst_id = FlstIdParsed::from_str(&sf.flst_id).parse_num();
            flst_id.as_ref() == Some(&such_id)
        }).collect::<Vec<_>>();

        let wurde_veraendert = splitflaechen.iter().any(|s| s.alt != s.neu);

        if !wurde_veraendert {
            return Status::Bleibt;
        }
        
        let alte_wirtschaftsarten = splitflaechen.iter().filter_map(|s| TaggedPolygon::get_wirtschaftsart(&s.alt)).collect::<BTreeSet<_>>();
        let neue_wirtschaftsarten = splitflaechen.iter().filter_map(|s| TaggedPolygon::get_wirtschaftsart(&s.neu)).collect::<BTreeSet<_>>();
        let veraendert_2 = alte_wirtschaftsarten.symmetric_difference(&neue_wirtschaftsarten).collect::<BTreeSet<_>>();

        if veraendert_2.is_empty() {
            Status::AenderungKeineBenachrichtigung
        } else {
            Status::AenderungMitBenachrichtigung
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct TextPlacement {
    pub kuerzel: String,
    pub status: TextStatus,
    pub pos: SvgPoint,
    pub ref_pos: SvgPoint,
    pub area: usize,
    pub poly: SvgPolygon,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum TextStatus {
    Old,
    New,
    StaysAsIs,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DstToLine {
    pub nearest_point: SvgPoint,
    pub distance: f64,
}

#[inline] fn sqr(x: f64) -> f64 { x * x }
#[inline] pub fn dist2(v: SvgPoint, w: SvgPoint) -> f64 { sqr(v.x - w.x) + sqr(v.y - w.y) }
#[inline] pub fn dist(v: SvgPoint, w: SvgPoint) -> f64 { dist2(v, w).sqrt() }
#[inline] pub fn dist_to_segment(p: SvgPoint, v: SvgPoint, w: SvgPoint) -> DstToLine { 
    
    let l2 = dist2(v, w);
    if (l2.abs() < 0.0001) {
        let dst = dist(p, v.clone());
        return DstToLine {
            nearest_point: v,
            distance: dst,
        };
    }

    let mut t = ((p.x - v.x) * (w.x - v.x) + (p.y - v.y) * (w.y - v.y)) / l2;
    t = 0.0_f64.max(1.0_f64.min(t));
    let nearest_point_on_line = SvgPoint { x: v.x + t * (w.x - v.x), y: v.y + t * (w.y - v.y) };
    DstToLine {
        nearest_point: nearest_point_on_line,
        distance: dist(p, nearest_point_on_line),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CorrectPointItem {
    NearPoint(SvgPoint),
    NearLine(DstToLine),
}

impl CorrectPointItem {
    pub fn get_point(&self) -> SvgPoint {
        use self::CorrectPointItem::*;
        match self {
            NearPoint(p) => *p,
            NearLine(dst) => dst.nearest_point,
        }
    }
    pub fn is_line(&self) -> bool {
        use self::CorrectPointItem::*;
        match self {
            NearPoint(_) => false,
            NearLine(_) => true,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
enum GetNearestPointFilter {
    Points,
    Lines,
    PointsAndLines,
}

impl Aenderungen {

    pub fn correct_point(
        p: &SvgPoint, 
        i: &[SvgPolygon], 
        maxdst_point: f64, 
        maxdst_line: f64, 
        allow_correctline: bool, 
        moved_points: &Vec<(SvgPoint, SvgPoint)>
    ) -> Option<SvgPoint> {
        if !allow_correctline {
            Self::get_nearest_point(p, i, maxdst_point, maxdst_line, GetNearestPointFilter::Points, moved_points)
        } else {
            let np1 = Self::get_nearest_point(p, i, maxdst_point, maxdst_line, GetNearestPointFilter::Points, moved_points);
            let np2 = Self::get_nearest_point(p, i, maxdst_point, maxdst_line, GetNearestPointFilter::Lines, moved_points);
            np1.or(np2)
        }
    }

    fn get_nearest_points(p: &SvgPoint, i: &[SvgPolygon], maxdst_point: f64, maxdst_line: f64, mode: GetNearestPointFilter, moved_points: &Vec<(SvgPoint, SvgPoint)>) -> Vec<(f64, CorrectPointItem)> {
        let mut near_points = Vec::new();
        
        for poly in i.iter() {
            near_points.append(&mut Self::get_points_near_point(p, &poly.outer_rings, maxdst_point, maxdst_line, mode));
            near_points.append(&mut Self::get_points_near_point(p, &poly.inner_rings, maxdst_point, maxdst_line, mode));
        }

        let mut near_points = near_points.into_iter().filter_map(|(dst, v)| {
            let current_target_point = v.get_point();
            if moved_points.iter().any(|(orig_point, _target_point)| orig_point.equals(&current_target_point)) {
                return None; // prevent "swapping" of point positions
            }
            if v.is_line() && dst.abs() < 0.001 {
                None
            } else {
                Some((dst, v))
            }
        }).collect::<Vec<_>>();

        near_points.sort_by(|a, b| a.0.total_cmp(&b.0));
        near_points
    }

    fn get_nearest_point(p: &SvgPoint, i: &[SvgPolygon], maxdst_point: f64, maxdst_line: f64, mode: GetNearestPointFilter, moved_points: &Vec<(SvgPoint, SvgPoint)>) -> Option<SvgPoint> {
        let np = Self::get_nearest_points(p, i, maxdst_point, maxdst_line, mode, moved_points);
        np.first().map(|p| p.1.get_point())
    }

    fn get_points_near_point(p: &SvgPoint, i: &[SvgLine], maxdst_point: f64, maxdst_line: f64, mode: GetNearestPointFilter) -> Vec<(f64, CorrectPointItem)> {
        let mut v = Vec::new();
        for line in i.iter() {
            let line: &crate::nas::SvgLine = line;
            for ab in line.points.windows(2) {
                match &ab {
                    &[a, b] => {
                        if mode == GetNearestPointFilter::Points || mode == GetNearestPointFilter::PointsAndLines {
                            let dist_ap = dist(*a, *p);
                            if dist_ap < maxdst_point {
                                v.push((dist_ap, CorrectPointItem::NearPoint(*a)));
                            }
    
                            let dist_bp = dist(*b, *p);
                            if dist_bp < maxdst_point {
                                v.push((dist_bp, CorrectPointItem::NearPoint(*b)));
                            }    
                        }
                        if mode == GetNearestPointFilter::Lines || mode == GetNearestPointFilter::PointsAndLines {
                            let dst = dist_to_segment(*p, *a, *b);
                            if dst.distance < maxdst_line {
                                v.push((dst.distance, CorrectPointItem::NearLine(dst)));
                            }
                        }
                    },
                    _ => { },
                }
            }
        }
        v
    }

    pub fn round_to_3decimal(&self) -> Aenderungen {
        Aenderungen {
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_definiert: self.na_definiert.clone(),
            na_polygone_neu: self.na_polygone_neu.iter().map(|(k, v)| {
                (k.clone(), PolyNeu {
                    nutzung: v.nutzung.clone(),
                    poly: v.poly.round_to_3dec(),
                })
            }).collect()
        }
    }

    pub fn clean_stage0(&self, maxdst_point: f64) -> Aenderungen {
        let mut changed_mut = self.round_to_3decimal();
        
        log_status(&format!("clean_stage0"));

        // deduplicate aenderungen
        let mut na_polygone_neu = BTreeMap::new();
        for (id, polyneu) in changed_mut.na_polygone_neu.iter() {
            let json = match serde_json::to_string(&polyneu) {
                Ok(o) => o,
                Err(_) => continue,
            };
            na_polygone_neu.insert(json, (id.clone(), polyneu.clone()));
        }
        changed_mut.na_polygone_neu = na_polygone_neu.values().map(|(k, v)| (k.clone(), v.clone())).collect();

        // join sequential points if 

        log_status(&format!("clean_stage0 1"));

        for (id, polyneu) in changed_mut.na_polygone_neu.iter_mut() {
            for ol in polyneu.poly.outer_rings.iter_mut() {
                *ol = clean_line(ol, maxdst_point);
            }
            for il in polyneu.poly.outer_rings.iter_mut() {
                *il = clean_line(il, maxdst_point);
            }
        }

        log_status(&format!("clean_stage0 2"));

        fn clean_line(l: &SvgLine, dst: f64) -> SvgLine {
            let mut first_point = match l.points.get(0) {
                Some(s) => s.clone(),
                None => return SvgLine::default(),
            };
            let first_point_copy = first_point.clone();

            let mut all_points = vec![first_point.clone()];
            for p in l.points.iter().skip(1) {
                if p.dist(&first_point) > dst {
                    all_points.push(*p);
                    first_point = *p;
                }
            }

            // handle last point
            if first_point.dist(&first_point_copy) > dst {
                all_points.push(first_point.clone());
            }

            all_points.push(first_point_copy.clone());

            all_points.dedup_by(|a, b| a.equals(b));
            
            SvgLine {
                points: all_points,
            }
        }

        changed_mut.round_to_3decimal()
    }

    pub fn clean_stage1(&self, log: &mut Vec<String>, maxdst_point: f64, maxdst_line: f64) -> Aenderungen {
        let mut changed_mut = self.clean_stage0(maxdst_point);

        let mut modified_tree = changed_mut.na_polygone_neu.clone();

        log_status(&format!("cleaning stage1 points, maxdst_point = {maxdst_point}, maxdst_line = {maxdst_line}"));

        let mut moved_points = Vec::new();

        // 1. Änderungen miteinander verbinden
        for (id, polyneu) in changed_mut.na_polygone_neu.iter_mut() {
            let mut modified = false;

            let changes_list = modified_tree.iter().filter_map(|(k, s)| {
                if k == id { None } else { Some(s.poly.clone()) }
            }).collect::<Vec<_>>();

            let changes_btree = QuadTree::new(
                changes_list.iter().enumerate().map(|(i, p)| (quadtree_f32::ItemId(i), quadtree_f32::Item::Rect(p.get_rect())))
            );

            for line in polyneu.poly.outer_rings.iter_mut() {
                for p in line.points.iter_mut() {

                    let p_orig = p.clone();
                    let p_rect = p.get_rect(maxdst_point.max(maxdst_line));
                    let overlap = changes_btree.get_ids_that_overlap(&p_rect)
                    .into_iter()
                    .filter_map(|i| changes_list.get(i.0).cloned())
                    .collect::<Vec<_>>();
                
                    if let Some(newp) = Self::correct_point(p, &overlap, maxdst_point, maxdst_line, true, &moved_points) {
                        *p = newp;
                        modified = true;
                        moved_points.push((p_orig, newp));
                    }
                }
            }
            
            if (modified) {
                modified_tree.insert(id.clone(), polyneu.clone());
            }
        }

        log_status(&format!("moved {} points", moved_points.len()));
        changed_mut.round_to_3decimal()
    }

    // 2: Punkte einfügen auf Linien, die nahegelegenen Änderungen liegen
    pub fn clean_stage2(
        &self, 
        log: &mut Vec<String>, 
        maxdst_line: f64, 
        maxdst_line2: f64,
        maxdev_followline: f64,
    ) -> Aenderungen {

        let mut changed_mut = self.round_to_3decimal();
        log.push(format!("Änderungen auf Änderungen (maxdst_line = {maxdst_line}, maxdst_line2 = {maxdst_line2}, maxdev_followline = {maxdev_followline})"));

        let mut total_modified = 0;
        let mut total_cleaned = BTreeSet::new();

        'outer: loop {
            let aenderungen_quadtree = NasXmlQuadTree::from_aenderungen(&changed_mut);
            let mut polys_modified = 0;

            for (id, polyneu) in changed_mut.na_polygone_neu.iter_mut() {
                let mut local_modified = 0;
                if total_cleaned.contains(id) {
                    continue;
                }

                for line in polyneu.poly.outer_rings.iter_mut() {
                    
                    let orig_points_len = line.points.len();
                    let mut nextpoint;
                    let mut newpoints = match line.points.get(0) {
                        Some(s) => {
                            nextpoint = s.clone();
                            vec![s.clone()]
                        },
                        None => continue,
                    };
    
                    for p in line.points.iter().skip(1) {
                        let start = nextpoint.clone();
                        let end = p;
                        newpoints.extend(aenderungen_quadtree.get_line_between_points(&start, end, log, maxdst_line, maxdst_line2, maxdev_followline, Some(id.clone())).into_iter());
                        newpoints.push(*end);
                        nextpoint = *end;
                    }
    
                    newpoints.dedup_by(|a, b| a.equals(b));
    
                    let newpoints_len = newpoints.len();
                    if newpoints_len != orig_points_len {
                        local_modified += newpoints_len.saturating_sub(orig_points_len);
                        log.push(format!("{id}: insert {} points", newpoints_len.saturating_sub(orig_points_len)));
                    }

                    line.points = newpoints;
                }

                total_modified += local_modified;
                polys_modified += local_modified;
                if local_modified != 0 {
                    total_cleaned.insert(id.clone());
                    continue 'outer;
                }
            }

            if polys_modified == 0 {
                break 'outer;
            }
        }

        let all_points_vec = changed_mut.na_polygone_neu.iter()
        .flat_map(|(k, q)| {
            let mut v = q.poly.outer_rings.iter().flat_map(|p| p.points.clone()).collect::<Vec<_>>();
            v.extend(q.poly.inner_rings.iter().flat_map(|p| p.points.clone()));
            v.into_iter()
        }).collect::<Vec<_>>();

        let qt_len = all_points_vec.len().saturating_div(20).max(500);
        let all_points_btree = QuadTree::new_with_max_items_per_quad(
            all_points_vec.iter().enumerate()
            .map(|(i, v)| (quadtree_f32::ItemId(i), quadtree_f32::Item::Rect(v.get_rect(0.01)))), 
            qt_len,
        );

        for v in changed_mut.na_polygone_neu.values_mut() {
            let ol = v.poly.outer_rings.iter().map(|p| Self::insert_points(p, &all_points_btree, &all_points_vec)).collect::<Vec<_>>();
            let il = v.poly.inner_rings.iter().map(|p| Self::insert_points(p, &all_points_btree, &all_points_vec)).collect::<Vec<_>>();
            v.poly = SvgPolygon {
                outer_rings: ol,
                inner_rings: il,
            };
        }

        changed_mut.round_to_3decimal()

    }

    fn insert_points(l: &SvgLine, btree: &quadtree_f32::QuadTree, ap_vec: &[SvgPoint]) -> SvgLine {
        let first = match l.points.first() {
            Some(s) => s.clone(),
            None => return SvgLine::default(),
        };
        let mut finalized = vec![first];
        for p in l.points.windows(2) {
            let (a, b) = match p {
                &[a, b] => (a, b),
                _ => continue,
            };

            let all_points_to_question = btree
            .get_ids_contained_by(&points_to_rect(&(a, b)))
            .into_iter()
            .filter_map(|i| ap_vec.get(i.0))
            .collect::<Vec<_>>();

            let mut all_points_to_question = all_points_to_question
            .into_iter()
            .filter_map(|p| {
                let p = SvgPoint { x: p.x, y: p.y };
                if p.equals(&a) || p.equals(&b) {
                    return None;
                }
                let dst = dist_to_segment(p, a, b).distance.abs();
                if dst < 1.0 {
                    Some(p)
                } else {
                    None
                }
            }).collect::<Vec<_>>();

            all_points_to_question.sort_by(|d, e| a.dist(&d).total_cmp(&a.dist(&e)));

            for q in all_points_to_question {
                finalized.push(q);
            }

            finalized.push(b);
        }

        SvgLine { points: finalized }
    }

    // 3: Änderungen verbinden nach Typ, wenn sie sich gegenseitig berühren
    pub fn clean_stage25(&self) -> Aenderungen {
        self.clean_stage25_internal()
        .clean_stage1(&mut Vec::new(), 0.1, 0.1)
        .clean_stage25_internal()
        .move_lines_touching()
        .clean_stage25_internal()
    }

    fn move_lines_touching(&self) -> Aenderungen {

        let aenderungen_by_kuerzel = self.na_polygone_neu.iter().filter_map(|(id, k)| {
            let nutzung = k.nutzung.clone()?;
            Some((nutzung, k.poly.clone()))
        }).collect::<Vec<(_, _)>>();
        
        let mut aenderungen_by_kuerzel_map = BTreeMap::new();
        for (k, v) in aenderungen_by_kuerzel.into_iter() {
            aenderungen_by_kuerzel_map.entry(k.clone()).or_insert_with(|| Vec::new()).push(v);
        }

        let na_poly_clone = self.na_polygone_neu.clone();
        let mut touching_polys = BTreeMap::new();
        for (id, poly) in self.na_polygone_neu.iter() {
            touching_polys.extend(na_poly_clone.iter().filter_map(|(idn, q)| {
                if idn == id {
                    return None;
                }
                if q.nutzung.is_none() || poly.nutzung.is_none() {
                    return None;
                }
                if q.nutzung != poly.nutzung {
                    return None;
                }

                let relate = nas::relate(&poly.poly, &q.poly, 1.0);

                if relate.touches_other_poly_outside() {
                    let sm = id.max(idn);
                    let lg = id.min(idn);
                    Some((sm.clone(), lg.clone()))
                } else {
                    None
                }
            }));
        }

        let mut na_poly_neu = self.na_polygone_neu.clone();

        for (a, b) in touching_polys.iter() {
            let poly_a = match self.na_polygone_neu.get(a) {
                Some(s) => s,
                None => continue,
            };
            let poly_b = match self.na_polygone_neu.get(b) {
                Some(s) => s,
                None => continue,
            };
            let triangle_points_b = translate_to_geo_poly(&poly_b.poly).0
            .iter().flat_map(|f| f.earcut_triangles()).map(|i| i.centroid())
            .map(|p| SvgPoint { x: p.x(), y: p.y() })
            .collect::<Vec<_>>();

            fn get_nearest_point(a: &SvgPoint, list_b: &[SvgPoint]) -> SvgPoint {
                let mut dst = f64::MAX;
                let mut point = a.clone();
                for b in list_b.iter() {
                    let dst_new = b.dist(a);
                    if dst_new < dst { 
                        dst = dst_new;
                        point = *b;
                    }
                }
                if a.equals(&point) {
                    point
                } else {
                    SvgPoint {
                        x: a.x + ((point.x - a.x) / 50.0),
                        y: a.y + ((point.y - a.y) / 50.0),
                    }
                }
            }

            fn mini_correct_line(a: &SvgLine, b: &SvgPolygon, list_b: &[SvgPoint]) -> SvgLine {
                let mut p_final = Vec::new();
                let p_coords = b.get_all_pointcoords_sorted();
                for p in a.points.iter() {
                    let p_test = [(p.x * 1000.0).round() as usize, (p.y * 1000.0).round() as usize];
                    if p_coords.contains(&p_test) {
                        p_final.push(*p);
                    } else if nas::point_is_on_any_line(p, b, 1.0) {
                        p_final.push(get_nearest_point(p, list_b));
                    } else {
                        p_final.push(*p);
                    }

                }

                SvgLine { points: p_final }
            }

            let or_new = poly_a.poly.outer_rings.iter().map(|l| mini_correct_line(l, &poly_b.poly, &triangle_points_b)).collect::<Vec<_>>();
            let ir_new = poly_a.poly.inner_rings.iter().map(|l| mini_correct_line(l, &poly_b.poly, &triangle_points_b)).collect::<Vec<_>>();

            na_poly_neu.get_mut(a).unwrap().poly = SvgPolygon {
                outer_rings: or_new,
                inner_rings: ir_new,
            };
        }    

        Aenderungen {
            na_definiert: self.na_definiert.clone(),
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_polygone_neu: na_poly_neu,
        }.round_to_3decimal()
    }

    pub fn clean_stage25_internal(&self) -> Aenderungen {

        log_status("clean_stage25_internal");

        let aenderungen_by_kuerzel = self.na_polygone_neu.iter().filter_map(|(id, k)| {
            let nutzung = k.nutzung.clone()?;
            Some((nutzung, k.poly.clone()))
        }).collect::<Vec<(_, _)>>();
        
        let mut aenderungen_by_kuerzel_map = BTreeMap::new();
        for (k, v) in aenderungen_by_kuerzel.into_iter() {
            aenderungen_by_kuerzel_map.entry(k.clone()).or_insert_with(|| Vec::new()).push(v);
        }

        log_status("clean_stage25_internal 1");

        let joined = aenderungen_by_kuerzel_map.iter().flat_map(|(kuerzel, v)| {
            let joined = match join_polys(&v, false, false) {
                Some(s) => s,
                None => return Vec::new(),
            };

            joined.outer_rings.iter().map(|l| (uuid(), PolyNeu {
                poly: SvgPolygon::from_line(l),
                nutzung: Some(kuerzel.clone()),
            })).collect()
        }).collect::<BTreeMap<_, _>>();

        log_status("clean_stage25_internal 2");

        let mut aenderungen_alt = self.na_polygone_neu.iter().filter_map(|(id, v)| {
            if v.nutzung.is_none() {
                Some((id.clone(), v.clone()))
            } else {
                None
            }
        }).collect::<BTreeMap<_, _>>();

        aenderungen_alt.extend(joined.into_iter());

        log_status("clean_stage25_internal 3");

        Aenderungen {
            na_definiert: self.na_definiert.clone(),
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_polygone_neu: aenderungen_alt,
        }.round_to_3decimal()
    }
    
    // 2. Naheliegende Punktkoordinaten auf Flurstücks- / Nutzungsartengrenzen ziehen
    pub fn clean_stage3(&self, split_nas: &SplitNasXml, log: &mut Vec<String>, maxdst_point: f64, maxdst_line: f64) -> Aenderungen {
        let qt = split_nas.create_quadtree();

        let mut moved_points = Vec::new();
        let mut changed_mut = self.clone();
        for (_id, polyneu) in changed_mut.na_polygone_neu.iter_mut() {
            for line in polyneu.poly.outer_rings.iter_mut() {
                for p in line.points.iter_mut() {
                    let p_orig = p.clone();
                    let overlapping_flst_nutzungen = qt.get_overlapping_flst(&p.get_rect(maxdst_line.max(maxdst_point)));
                    let overlapping_flst_nutzungen = overlapping_flst_nutzungen.into_iter().map(|(id, v)| v.poly).collect::<Vec<_>>();
                    if let Some(newp) = Self::correct_point(p, &overlapping_flst_nutzungen, maxdst_point, maxdst_line, true, &moved_points) {
                        *p = newp;
                        moved_points.push((p_orig, newp));
                    }
                }
            }
        }

        changed_mut.round_to_3decimal()

    }

    // 3: Punkte einfügen auf Linien, die nahe Flurstücks- / Nutzungsartengrenzen liegen
    pub fn clean_stage4(
        &self, 
        original_xml: &NasXMLFile, 
        log: &mut Vec<String>, 
        maxdst_line: f64, 
        maxdst_line2: f64,
        maxdev_followline: f64,
    ) -> Aenderungen {
        let mut changed_mut = self.round_to_3decimal();
        let nas_quadtree = original_xml.create_quadtree();

        for (id, polyneu) in changed_mut.na_polygone_neu.iter_mut() {
            for line in polyneu.poly.outer_rings.iter_mut() {
                
                let mut nextpoint;
                let mut newpoints = match line.points.get(0) {
                    Some(s) => {
                        nextpoint = s.clone();
                        vec![s.clone()]
                    },
                    None => continue,
                };

                for p in line.points.iter().skip(1) {
                    let start = nextpoint.clone();
                    let end = p;
                    newpoints.extend(nas_quadtree.get_line_between_points(&start, end, log, maxdst_line, maxdst_line2, maxdev_followline, Some(id.clone())).into_iter());
                    newpoints.push(*end);
                    nextpoint = *end;
                }

                newpoints.dedup_by(|a, b| a.equals(b));

                line.points = newpoints;
            }
        }

        changed_mut.round_to_3decimal()

    }

    pub fn clean_nas(&self) -> Aenderungen {
        Aenderungen {
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_definiert: self.na_definiert.clone(),
            na_polygone_neu: self.na_polygone_neu.iter().map(|(k, v)| {
                (k.clone(), PolyNeu {
                    nutzung: v.nutzung.clone(),
                    poly: crate::nas::cleanup_poly(&v.poly),
                })
            }).collect()
        }
    }

    pub fn remerge_lines(&self) -> Aenderungen {

        // deduplicate and un-merge all lines
        let mut all_rings = BTreeMap::new();
        for s in self.na_polygone_neu.iter() {
            for or in s.1.poly.outer_rings.iter() {
                all_rings.insert(or.get_hash(), or);
            }
            for ir in s.1.poly.inner_rings.iter() {
                all_rings.insert(ir.get_hash(), ir);
            }
        }
        
        // recombine lines based on inside / outside (set nutzung = None for now)
        let mut recombined: Vec<Vec<&&SvgLine>> = Vec::new();
        'outer: for (_, line) in all_rings.iter() {
            for r in recombined.iter_mut() {
                let first_line: SvgLine = match r.first().cloned() {
                    Some(s) => (*s).clone(),
                    None => break,
                };
                if nas::line_contained_in_line(&first_line, line) {
                    // line is an inner ring, push to end
                    r.push(line);
                    continue 'outer;
                } else if nas::line_contained_in_line(&line, &first_line) {
                    // line is an outer ring, push to start
                    r.push(line);
                    r.reverse();
                    continue 'outer;
                }
            }

            recombined.push(vec![line]);
        }

        let mut polygons_recombined = recombined.iter().filter_map(|s| Some(SvgPolygon {
            outer_rings: vec![(**s.get(0)?).clone()],
            inner_rings: s.iter().skip(1).map(|s| (**s).clone()).collect(),
        })).collect::<Vec<_>>();

        let mut new_inner_rings_polys = polygons_recombined.iter().flat_map(|p| {
            p.inner_rings.iter().map(|l| SvgPolygon::from_line(l))
        }).collect::<Vec<_>>();

        polygons_recombined.append(&mut new_inner_rings_polys);

        // TODO: add nutzungen again based on triangle test with original changes

        Aenderungen {
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_definiert: self.na_definiert.clone(),
            na_polygone_neu: polygons_recombined.into_iter().map(|p| {
                (uuid(), PolyNeu {
                    nutzung: None,
                    poly: p,
                })
            }).collect(),
        }
    }

    // Subtrahiere Änderungen, die über Änderungen liegen
    pub fn clean_stage5(&self, split_nas: &SplitNasXml, log: &mut Vec<String>) -> Aenderungen {
        
        let mut changed_mut = self.clone();
        let mut geaendert = BTreeMap::new();

        for (pid, pn) in changed_mut.na_polygone_neu.iter() {
            let pn_rect = pn.poly.get_rect();
            let p_nutzung = match pn.nutzung.clone() {
                Some(s) => s,
                None => continue,
            };
            let nak = match TaggedPolygon::get_nutzungsartenkennung(&p_nutzung) {
                Some(s) => s,
                None => continue,
            };

            let higher_order_polys = changed_mut.na_polygone_neu.iter()
            .filter_map(|(id, v)| {
                let nutzung = v.nutzung.clone()?;
                Some((nutzung, id.clone(), &v.poly))
            })
            .filter_map(|(k, id, s)| {
                if TaggedPolygon::get_nutzungsartenkennung(&k)? > nak { 
                    Some((k, id, s)) 
                } else { 
                    None 
                }
            })
            .filter(|(_, id, _)|  id != pid)
            .filter(|(k, id, v)| v.get_rect().overlaps_rect(&pn_rect))
            .map(|s| s.2)
            .collect::<Vec<_>>();

            if !higher_order_polys.is_empty() {
                let subtracted = subtract_from_poly(&pn.poly, &higher_order_polys);
                geaendert.insert(pid.clone(), PolyNeu {
                    nutzung: pn.nutzung.clone(),
                    poly: subtracted,
                });
            }
        }

        for (id, np) in geaendert.into_iter() {
            changed_mut.na_polygone_neu.insert(id.to_string(), np);
        }

        changed_mut.round_to_3decimal().deduplicate()

    }

    fn clean_internal(&self) -> Aenderungen {
        Aenderungen {
            na_definiert: self.na_definiert.clone(),
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_polygone_neu: self.na_polygone_neu
            .iter()
            .map(|(k, v)| {
                (k.clone(), PolyNeu {
                    nutzung: v.nutzung.clone(),
                    poly: v.poly.round_to_3dec(),
                })
            })
            .filter(|s| !s.1.poly.is_zero_area())
            .collect()
        }
    }

    pub fn deduplicate(&self) -> Self {

        let s = self.round_to_3decimal();

        let mut aenderung_2 = BTreeMap::new();
        for (k, v) in s.na_polygone_neu.iter() {

            let a = match serde_json::to_string(&v.poly) {
                Ok(o) => o,
                Err(_) => continue,
            };

            aenderung_2.insert(a, (k.clone(), v.nutzung.clone()));
        }

        Self {
            gebaeude_loeschen: s.gebaeude_loeschen.clone(),
            na_definiert: s.na_definiert.clone(),
            na_polygone_neu: aenderung_2.into_iter()
            .filter_map(|(k, (v0, v1))| Some((v0, PolyNeu { 
                nutzung: v1,
                poly: serde_json::from_str(&k).ok()?
            }))).collect(),
        }
    }

    
    pub fn show_splitflaechen(
        &self,
        split_nas: &SplitNasXml,
        original_xml: &NasXMLFile, 
        csv: &CsvDataType,
    ) -> Aenderungen {

        let intersections = AenderungenClean {
            nas_xml_quadtree: split_nas.create_quadtree(),
            aenderungen: self.clone(),
        }.get_aenderungen_intersections(original_xml, crate::get_main_gemarkung(csv));

        Aenderungen {
            gebaeude_loeschen: self.gebaeude_loeschen.clone(),
            na_definiert: self.na_definiert.clone(),
            na_polygone_neu: intersections.0.iter().enumerate().map(|(i, is)| {
                let id = format!("{i}: {k} :: {n}", k = is.alt, n = is.neu);
                (id, PolyNeu {
                    nutzung: Some(is.neu.clone()),
                    poly: is.poly_cut.clone()
                })
            }).collect()
        }.round_to_3decimal()
    }

}

pub fn render_main(
    projekt_info: &ProjektInfo,
    risse: &Risse,
    uidata: &UiData,
    csv: &CsvDataType, 
    aenderungen: &Aenderungen,
) -> String {
    let map = format!("
        <div id='__application-main-container' style='display:flex;flex-grow:1;position:relative;overflow:hidden;'>
            <div id='__application_main-overlay-container' style='width:400px;max-width:400px;min-width:400px;display:flex;flex-grow:1;flex-direction:column;box-shadow:0px 0px 10px black;z-index:999;'>
                <div id='switch-content' style='display: flex;flex-direction: row;flex-grow: 1;max-height: 30px;min-height:30px;'>
                    {render_switch_content}
                </div>
                <div style='background:white;padding:20px;pointer-events:all;min-width:400px;display:flex;flex-grow:1'>
                    <div id='__application_project_content' class='csv-scrollbox' style='max-height:80vh;flex-grow: 1;overflow: scroll;display: flex;flex-direction: column;'>
                        {content}
                    </div>
                </div>
            </div>
            <div id='mapcontainer' style='display:flex;flex-grow:1;flex-direction:row;z-index:0;'>
                <div id='map' style='width:100%;height:100%;'></div>
            </div>
        </div>
    ",
        render_switch_content = render_switch_content(uidata),
        content = if uidata.secondary_content.unwrap_or_default() {
            render_secondary_content(&aenderungen)
        } else {
            render_project_content(projekt_info, risse, csv, aenderungen, uidata, &SplitNasXml::default()) 
        },
    );
    normalize_for_js(map) // TODO
}

pub fn render_switch_content(uidata: &UiData) -> String {
    if uidata.tab.unwrap_or_default() != 0 {
        return String::new();
    }
    let sec_active = if uidata.secondary_content.unwrap_or_default() { "active" } else { "" };
    let prim_active = if !uidata.secondary_content.unwrap_or_default() { "active" } else { "" };
    format!("
        <div class='project-content-switch {prim_active}' onclick='selectContent(0);'>Flurstücke</div>
        <div class='project-content-switch {sec_active}' onclick='selectContent(1);'>Änderungen</div>
    ")
}

pub fn render_secondary_content(aenderungen: &Aenderungen) -> String {

    let mut html = "<div id='aenderungen-container'>".to_string();
    
    html += "<h2>Gebäude löschen</h2>";
    html += "<div id='zu-loeschende-gebaeude'>";
    for (k, gebaeude_id) in aenderungen.gebaeude_loeschen.iter().rev() {
        let gebaeude_id = &gebaeude_id.gebaeude_id;
        html.push_str(&format!(
            "<div class='__application-aenderung-container' id='gebaeude-loeschen-{gebaeude_id}' data-gebaeude-id='{gebaeude_id}'>
                <div style='display:flex;'>
                    <p class='__application-zoom-to' onclick='zoomToGebaeudeLoeschen(event);' data-gebaeude-id='{gebaeude_id}'>[Karte]</p>
                    <p style='color: white;font-weight: bold;' data-gebaeude-id='{gebaeude_id}'>{gebaeude_id}</p>
                </div>
                <p class='__application-secondary-undo' onclick='gebaeudeLoeschenUndo(event);' data-gebaeude-id='{k}'>X</p>
            </div>"
        ));
    }
    html += "</div>";

    html += "<h2><span style='display: flex;flex-direction: row;justify-content: space-between;flex-grow: 1;'>Neue Nutzungen <p style='text-decoration:underline;cursor:pointer;' onclick='nutzungenSaeubern(event);' data-nutzung-id=''>[alle bereinigen]</p></span></h2>";
    html += "<div id='neue-na'>";
    for (new_poly_id, polyneu) in aenderungen.na_polygone_neu.iter().rev() {
        let select_nutzung = render_select(&polyneu.nutzung, "changeSelectPolyNeu", &new_poly_id, "aendern-poly-neu");
        let new_poly_id_first_chars = new_poly_id.split("-").next().unwrap_or("");
        let new_poly_id_first_chars = new_poly_id_first_chars.chars().take(10).collect::<String>();
        html.push_str(&format!(
            "<div class='na-neu' id='na-neu-{new_poly_id}' data-new-poly-id='{new_poly_id}'>
                <div style='display:flex;'>
                    <p class='__application-zoom-to' onclick='zoomToPolyNeu(event);' data-poly-neu-id='{new_poly_id}'>[Karte]</p>
                    <p class='__application-zoom-to' onclick='nutzungenSaeubern(event);' data-nutzung-id='{new_poly_id}' data-poly-neu-id='{new_poly_id}'>[ber.]</p>
                    <p style='color: white;font-weight: bold;' data-poly-neu-id='{new_poly_id}'>{new_poly_id_first_chars}</p>
                </div>
                <div style='display:flex;'>
                    {select_nutzung}
                    <p class='__application-secondary-undo' onclick='polyNeuUndo(event);' data-poly-neu-id='{new_poly_id}' style='margin-left: 10px;display: flex;align-items: center;'>X</p>
                </div>
            </div>"
        ));
    }
    html += "</div>";

    html += "</div>";
    html 
}

pub fn render_select(selected: &Option<String>, function: &str, id: &str, html_id: &str) -> String {
    let map = crate::get_map();
    let mut s = format!("<select id='{html_id}-{id}' onchange='{function}(event);' data-id='{id}'>");
    s.push_str(&format!("<option {selected} value='NOTDEFINED'>nicht defin.</option>", selected = if selected.is_none() { " selected='selected' " } else { "" }));
    for k in map.keys() {
        let selected = if selected.as_deref() == Some(k) { " selected='selected' " } else { "" };
        s.push_str(&format!("<option {selected}>{k}</option>"));
    }
    s += "</select>";
    s
}

pub fn render_project_content(
    projekt_info: &ProjektInfo, 
    risse: &Risse, 
    csv: &CsvDataType, 
    aenderungen: &Aenderungen, 
    uidata: &UiData, 
    split_fs: &SplitNasXml
) -> String {
    let s = match uidata.tab {
        Some(2) => render_risse_ui(projekt_info, risse, &csv, aenderungen),
        _ => render_csv_editable(&csv, aenderungen, uidata.render_out.unwrap_or_default(), &uidata.selected_edit_flst, Some(split_fs)),
    };
    normalize_for_js(s)
}

fn render_risse_ui(
    projekt_info: &ProjektInfo,
    risse: &Risse,
    csv: &CsvDataType, 
    aenderungen: &Aenderungen, 
) -> String {
    let render_config_field = |(name, value, id): (&str, &str, &str)| {
        format!("<div class='ri-config-field'>
            <label for='projekt-info-{id}' style='bold'>{name}</label>
            <input type='text' id='projekt-info-{id}' oninput='projectInfoEdit(event);' onchange='projectInfoEdit(event);' data-id='{id}' value='{value}'>
        </div>")
    };

    format!(
        "<div class='risse-wrapper-container'>
            <div class='projekt-info'>
                {antragsnr}
                {katasteramt}
                {vermessungsstelle}
                {erstellt_durch}
                {beruf_kuerzel}
                {gemeinde}
                {gemarkung}
                {bearbeitung_beendet_am}
                {alkis_akt}
                {ortho_akt}
                {feldbloecke_vom}
            </div>
            <div id='risse' style='display:flex;flex-direction:column;'>
                <h2 style='font-size:16px;font-weight:bold;margin-top:20px;'>Risse</h2>
                <button onclick='rissNeu(event);' style='margin: 10px 0px;display: flex;padding: 10px;border-radius: 5px;cursor: pointer;background: #828295;color: white;border: 1px solid black;'>Neuen Riss anlegen</button>
                {risse}
            </div>
        </div>",
        antragsnr = render_config_field(("Antragsnummer", &projekt_info.antragsnr, "antragsnr")),
        katasteramt = render_config_field(("Katasteramt", &projekt_info.katasteramt, "katasteramt")),
        vermessungsstelle = render_config_field(("Vermessungsstelle", &projekt_info.vermessungsstelle, "vermessungsstelle")),
        erstellt_durch = render_config_field(("Erstellt durch", &projekt_info.erstellt_durch, "erstellt_durch")),
        beruf_kuerzel = render_config_field(("Beruf (Kürzel)", &projekt_info.beruf_kuerzel, "beruf_kuerzel")),
        gemeinde = render_config_field(("Gemeinde", &projekt_info.gemeinde, "gemeinde")),
        gemarkung = render_config_field(("Gemarkung", &projekt_info.gemarkung, "gemarkung")),
        bearbeitung_beendet_am = render_config_field(("Bearb. beendet am", &projekt_info.bearbeitung_beendet_am, "bearbeitung_beendet_am")),
        alkis_akt = render_config_field(("ALKIS Daten vom", &projekt_info.alkis_aktualitaet, "alkis_aktualitaet")),
        ortho_akt = render_config_field(("Orthofoto vom", &projekt_info.orthofoto_datum, "orthofoto_datum")),
        feldbloecke_vom = render_config_field(("Feldblöcke vom", &projekt_info.gis_feldbloecke_datum, "gis_feldbloecke_datum")),
        risse = risse.iter().enumerate().map(|(riss_num, (id, rc))| {
            format!("<div class='riss-ui-wrapper' id='riss-{id}' style='display: flex;margin-bottom: 10px;flex-direction: column;padding: 10px;background: #cccccc;border-radius: 3px;'>

                <div class='row' style='display: flex;justify-content: space-between;padding: 5px 0px;'>
                    <p style='font-size: 14px;font-weight: bold;'>Riss {riss_num}</p>
                    <p class='__application-secondary-undo' style='margin-left: 10px;display: flex;align-items: center;' onclick='rissLoeschen(event);' data-riss-id='{id}'>X</p>
                </div>
                <div class='row' style='display: flex;justify-content: space-between;padding: 5px 0px;'>
                    <div>
                        <label for='projekt-info-{id}' style='font-weight: bold;margin-right: 5px;'>Breite:</label>
                        <input id='riss-{id}-width' type='number' style='max-width: 100px;margin-right:0px;' value='{width}' data-riss-id='{id}' data-input-id='width' oninput='changeRiss(event);' onchange='changeRiss(event);'></input>
                    </div>
                    <div>
                        <button onclick='switchRissWh(event);' data-riss-id='{id}'> &lt;&gt;</button>
                    </div>
                    <div>
                        <label for='projekt-info-{id}' style='font-weight: bold;margin-right: 5px;'>Höhe:</label>
                        <input id='riss-{id}-height' style='max-width: 100px;margin-right:0px;' type='number' value='{height}' data-riss-id='{id}' data-input-id='height' oninput='changeRiss(event);' onchange='changeRiss(event);'></input>
                    </div>
                </div>

                <div class='row' style='display: flex;justify-content: space-between;padding: 5px 0px;align-items:center;'>
                    <label for='projekt-info-{id}' style='font-weight: bold;margin-right: 5px;'>Maßstab:</label>
                    <input id='riss-{id}-scale' type='number' value='{scale}' style='display:flex;flex-grow:1;margin-right:0px;' data-riss-id='{id}' data-input-id='scale' oninput='changeRiss(event);' onchange='changeRiss(event);'></input>
                </div>

                <button id='riss-{id}-rissgebiet-zeichnen' style='margin-top:10px;padding: 5px;cursor:pointer;' data-riss-id='{id}' data-input-id='scale' onclick='showHideRissGebiet(event);'>Rissgebiet zeichnen</button>

                <!--
                <button id='riss-{id}-flm-setzen' style='margin-top:10px;padding: 5px;cursor:pointer;' data-riss-id='{id}' data-input-id='scale' onclick='showHideFlurMarker(event);'>Flurmarker setzen</button>
                <button id='riss-{id}-np-setzen' style='margin-top:10px;padding: 5px;cursor:pointer;' data-riss-id='{id}' onclick='showHideNordpfeil(event);'>Nordpfeil setzen</button>
                <button id='riss-{id}-anschluss-setzen' style='margin-top:10px;padding: 5px;cursor:pointer;' data-riss-id='{id}' onclick='showHideAnschlussRisse(event);'>Anschlussrisse setzen</button>
                <button id='riss-{id}-label-verschieben' style='margin-top:10px;padding: 5px;cursor:pointer;' data-riss-id='{id}' onclick='showHideLabel(event);'>Label verschieben</button>
                -->
            </div>",
                width = rc.width_mm,
                height = rc.height_mm,
                scale = rc.scale,
                riss_num = riss_num + 1,
            )
        }).collect::<Vec<_>>().join("")
    )
}

fn render_csv_editable(
    csv: &CsvDataType, 
    aenderungen: &Aenderungen,
    filter_out_bleibt: bool, 
    selected_edit_flst: &str,
    split_fs: Option<&SplitNasXml>,
) -> String {

    let selected_edit_flst = selected_edit_flst.replace("_", "");

    let content = csv.iter()
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
            selected_edit_flst.starts_with(k) 
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
        split_nas = if selected {
            match split_fs.and_then(|sn| sn.flurstuecke_nutzungen.get(&flstidparsed.format_start_str())) {
                None => String::new(),
                Some(s) => {
                    format!(
                        "<div class='nutzung-veraendern'>{}</div>", 
                        s.iter().filter_map(|tp| {
                            let ax_ebene = tp.attributes.get("AX_Ebene")?;
                            let ax_flurstueck = flstidparsed.format_start_str();
                            let cut_obj_id = tp.attributes.get("id")?;
                            let objid_total = format!("{ax_flurstueck}:{ax_ebene}:{cut_obj_id}");
                            let quadratmeter = tp.attributes.get("BerechneteGroesseM2").cloned().unwrap_or("0".to_string());
                            if quadratmeter == "0" {
                                return None;
                            }
                            let auto_kuerzel = tp.get_auto_kuerzel(ax_ebene);
                            let auto_kuerzel_str = auto_kuerzel.as_ref().unwrap_or(ax_ebene);
                            Some(format!(
                                "<div><p>{quadratmeter}m² {auto_kuerzel_str}</p>{}</div>", 
                                render_select(&
                                    aenderungen.na_definiert.get(&objid_total).cloned()
                                    .or(auto_kuerzel.clone())
                                , "nutzungsArtAendern", &objid_total, "nutzungsart-aendern")
                            ))
                        }).collect::<Vec<_>>().join("")
                    )
                }
            }
        } else {
            String::new()
        }
    ))
    }).collect::<Vec<_>>().join("");

    content
}

pub fn normalize_for_js(s: String) -> String {
    s.lines()
        .map(|s| s.trim().replace('`', "'"))
        .collect::<Vec<_>>()
        .join("")
}
