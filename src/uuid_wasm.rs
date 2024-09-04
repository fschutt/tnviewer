use rand::{Rng, SeedableRng};
use serde_derive::Serialize;
use serde_derive::Deserialize;
use wasm_bindgen::prelude::*;
use std::char;
use rand_xorshift::XorShiftRng;

use crate::pdf::Konfiguration;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
    fn update_export_status(s: String);
    fn export_status_clear();
}

pub fn js_random() -> f64 {
    random().max(0.0).min(1.0)
}

pub fn log_status_clear() {
    export_status_clear();
}

pub fn log_status(s: &str) {
    update_export_status(s.trim().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchWmsImageRequest {
    pub max_x: f64,
    pub max_y: f64,
    pub min_x: f64,
    pub min_y: f64,
    pub width_px: usize,
    pub height_px: usize,
}

pub async fn get_wms_image(config: &Konfiguration, obj: FetchWmsImageRequest) -> Option<printpdf::Image> {

    let mut url = config.map.dop_source.clone()?;
    url += "&SERVICE=WMS";
    url += "&REQUEST=GetMap";
    url += "&VERSION=1.1.1";
    url += format!("&LAYERS={}", config.map.dop_layers.clone().unwrap_or_default()).as_str();
    url += "&STYLES=";
    url += "&FORMAT=image%2Fpng";
    url += "&TRANSPARENT=false";
    url += format!("&HEIGHT={}", obj.height_px).as_str();
    url += format!("&WIDTH={}", obj.width_px).as_str();
    url += "&MAXNATIVEZOOM=25";
    url += "&SRS=EPSG%3A25833";
    url += format!("&BBOX={},{},{},{}", obj.min_x, obj.min_y, obj.max_x, obj.max_y).as_str();

    web_sys::console::log_1(&format!("reqwest fetching url {url}").into());

    let bytes = reqwest::get(&url).await.ok()?.bytes().await.ok()?.as_ref().to_vec();

    web_sys::console::log_1(&format!("ok received image {} bytes", bytes.len()).into());
    let format = match image::guess_format(&bytes){
        Ok(o) => o,
        Err(e) => {
            web_sys::console::log_1(&format!("failed image format: {} {:?}", e.to_string(), bytes.iter().take(10).collect::<Vec<_>>()).into());
            return None; 
        }
    };
    let decoded = match image::load_from_memory_with_format(&bytes , format) {
        Ok(o) => o,
        Err(e) => {
            web_sys::console::log_1(&format!("error 1: {}", e.to_string()).into());
            return None;
        }
    };
    web_sys::console::log_1(&format!("png decoder ok").into());
    let i = printpdf::Image::from_dynamic_image(&decoded);
    web_sys::console::log_1(&format!("image ok").into());
    Some(i)
}

pub fn uuid() -> String {
    let seed = random();
    gen_uuid_with_xorshift(seed)
}

pub fn random_color() -> String {
    use random_color::color_dictionary::{ColorDictionary, ColorInformation};
    use random_color::{Color, Luminosity, RandomColor};

    RandomColor::new()
    .luminosity(Luminosity::Light) // Optional
    .seed((random() * 1000.0) as i64) // Optional
    .to_hex()
}

enum UuidElements {
    Random09AF,
    Random89AB,
    Hyphen,
    Version,
}

const UUID_V4_FORMAT: [UuidElements; 36] = [
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Hyphen,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Hyphen,
    UuidElements::Version,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Hyphen,
    UuidElements::Random89AB,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Hyphen,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
    UuidElements::Random09AF,
];

const ERROR_MAKE_CHAR: &str = "Error in making char";

fn make_bytes(value: f64) -> [u8; 16] {
    let bytes = value.to_bits();

    let b1: u8 = ((bytes >> 56) & 0xff) as u8;
    let b2: u8 = ((bytes >> 48) & 0xff) as u8;
    let b3: u8 = ((bytes >> 40) & 0xff) as u8;
    let b4: u8 = ((bytes >> 36) & 0xff) as u8;
    let b5: u8 = ((bytes >> 24) & 0xff) as u8;
    let b6: u8 = ((bytes >> 16) & 0xff) as u8;
    let b7: u8 = ((bytes >> 8) & 0xff) as u8;
    let b8: u8 = (bytes & 0xff) as u8;

    [b8, b7, b6, b5, b4, b3, b2, b1, b1, b2, b3, b4, b5, b6, b7, b8]
}

pub fn gen_uuid_with_xorshift(seed: f64) -> String {
    let bytes = make_bytes(seed);
    let mut rng = XorShiftRng::from_seed(bytes);
    
    // prevent duplication
    rng.gen_range(0.0..1.0);

    UUID_V4_FORMAT.into_iter()
        .map(|n| match n {
            UuidElements::Random09AF => {
                let random = rng.gen_range(0.0..1.0);
                char::from_digit((random * 16.) as u32, 16).expect(ERROR_MAKE_CHAR)
            }
            UuidElements::Random89AB => {
                let random = rng.gen_range(0.0..1.0);
                char::from_digit((random * 4.) as u32 + 8, 16).expect(ERROR_MAKE_CHAR)
            }
            UuidElements::Version => '4',
            UuidElements::Hyphen => '-',
        })
        .collect()
}
