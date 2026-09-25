//! Loading of embedded images: the application icon and the menu icon.

use std::io::{Cursor, Read};
use std::sync::Arc;

use eframe::egui;
use egui::IconData;
use flate2::read::GzDecoder;
use image::{ImageError, ImageFormat, ImageReader, load_from_memory};

// Embedded menu image bytes (required to be embedded)
#[cfg(not(target_os = "android"))]
pub const MENU_BYTES: &[u8] = include_bytes!("../assets/menu.png.gz");
#[cfg(not(target_os = "android"))]
pub const SETTINGS_BYTES: &[u8] = include_bytes!("../assets/settings.png.gz");

#[cfg(target_os = "android")]
pub const MENU_BYTES: &[u8] = include_bytes!("../../android/assets/menu.png.gz");
#[cfg(target_os = "android")]
pub const SETTINGS_BYTES: &[u8] = include_bytes!("../../android/assets/menu.png.gz");

#[allow(dead_code)]
pub fn ico_bytes_to_png_bytes(ico_data: &[u8]) -> Result<Vec<u8>, ImageError> {
    // Decode the ICO into a DynamicImage
    let img = load_from_memory(ico_data)?;

    // Create a Cursor that owns the vector and supports Seek
    let png_data = Vec::new();
    let mut cursor = Cursor::new(png_data);

    // Write the PNG into the Cursor
    img.write_to(&mut cursor, ImageFormat::Png)?;

    // Take back the inner vector with the PNG data
    Ok(cursor.into_inner())
}

/// Direct ICO → RGBA without PNG intermediate (avoids extra alloc + decode).
fn decode_ico_rgba(ico_data: &[u8]) -> Result<(Vec<u8>, u32, u32), ImageError> {
    let rgba = load_from_memory(ico_data)?.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Decodes the application icon into an egui texture (original colors).
pub fn load_icon_color_image() -> egui::ColorImage {
    let (rgba, w, h) = decode_ico_rgba(database::ICON_BYTES).expect("decode app icon");
    // Reconstruct RgbaImage view without re-decoding PNG
    let img = {
        use image::RgbaImage;
        RgbaImage::from_raw(w, h, rgba).expect("rgba size")
    };
    let (w, h) = (img.width() as usize, img.height() as usize);
    egui::ColorImage {
        size: [w, h],
        source_size: egui::Vec2::new(w as f32, h as f32),
        pixels: img
            .pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect(),
    }
}

#[allow(dead_code)]
pub fn load_icon_from_bytes(bytes: &[u8]) -> Result<Arc<IconData>, image::ImageError> {
    let img = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(Arc::new(IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }))
}

/// Fast path for ICO bytes directly → IconData without PNG round-trip.
pub fn load_icon_from_ico_bytes(ico_data: &[u8]) -> Result<Arc<IconData>, image::ImageError> {
    let (rgba, width, height) = decode_ico_rgba(ico_data)?;
    Ok(Arc::new(IconData {
        rgba,
        width,
        height,
    }))
}

pub fn load_asset_image(data: &[u8]) -> egui::ColorImage {
    // White with original alpha; actual color applied at draw time via tint()
    let mut bytes: Vec<u8> = vec![];

    let mut decoder = GzDecoder::new(data);
    decoder.read_to_end(&mut bytes).expect("Failded load asset");

    let img = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .expect("menu png format")
        .decode()
        .expect("decode menu")
        .to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut pixels = Vec::with_capacity(w * h);
    for p in img.pixels() {
        let a = p[3];
        if a < 10 {
            pixels.push(egui::Color32::TRANSPARENT);
        } else {
            pixels.push(egui::Color32::from_rgba_unmultiplied(255, 255, 255, a));
        }
    }
    egui::ColorImage {
        size: [w, h],
        source_size: egui::Vec2::new(w as f32, h as f32),
        pixels,
    }
}
