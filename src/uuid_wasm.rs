use rand::{Rng, SeedableRng};
use wasm_bindgen::prelude::*;
use std::char;
use rand_xorshift::XorShiftRng;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
    fn update_export_status(s: String);
    fn export_status_clear();
}

pub fn log_status_clear() {
    export_status_clear();
}

pub fn log_status(s: &str) {
    update_export_status(s.trim().to_string())
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
