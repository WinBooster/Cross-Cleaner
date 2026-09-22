use image::DynamicImage;
use image::codecs::bmp::BmpEncoder;
use image::codecs::gif::GifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::tiff::TiffEncoder;
use image::codecs::webp::WebPEncoder;
use std::io::{self, Cursor, Write};
use std::path::Path;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff"];
const JPEG_QUALITY: u8 = 85;

fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

pub fn optimize_single(path: &Path) -> Result<crate::custom_cleaners::GlobCleanStats, io::Error> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    eprintln!(
        "[image_optimize] path={} ext={:?} is_image={}",
        path.display(),
        ext,
        is_image_extension(&ext)
    );

    if !is_image_extension(&ext) {
        return Ok(crate::custom_cleaners::GlobCleanStats::default());
    }

    let original_size = std::fs::metadata(path)?.len();
    eprintln!("[image_optimize] original_size={}", original_size);

    let img = image::open(path).map_err(|e| {
        eprintln!("[image_optimize] open failed: {}", e);
        io::Error::new(io::ErrorKind::InvalidData, e)
    })?;

    let mut buf = Cursor::new(Vec::new());
    match ext.as_str() {
        "png" => encode_png(&mut buf, &img)?,
        "jpg" | "jpeg" => encode_jpeg(&mut buf, &img)?,
        "webp" => encode_webp(&mut buf, &img)?,
        "gif" => encode_gif(&mut buf, &img)?,
        "bmp" => encode_bmp(&mut buf, &img)?,
        "tiff" => encode_tiff(&mut buf, &img)?,
        _ => return Ok(crate::custom_cleaners::GlobCleanStats::default()),
    }

    let compressed = buf.into_inner();
    let new_size = compressed.len() as u64;

    eprintln!(
        "[image_optimize] new_size={} saved={}",
        new_size,
        original_size.saturating_sub(new_size)
    );

    if new_size >= original_size {
        return Ok(crate::custom_cleaners::GlobCleanStats::default());
    }

    std::fs::write(path, &compressed)?;

    let saved = original_size - new_size;
    eprintln!("[image_optimize] WRITTEN saved={}", saved);
    Ok(crate::custom_cleaners::GlobCleanStats::file(saved))
}

fn encode_png(w: &mut impl Write, img: &DynamicImage) -> io::Result<()> {
    let encoder = PngEncoder::new_with_quality(w, CompressionType::Best, FilterType::Adaptive);
    img.write_with_encoder(encoder).map_err(io::Error::other)
}

fn encode_jpeg(w: &mut impl Write, img: &DynamicImage) -> io::Result<()> {
    let encoder = JpegEncoder::new_with_quality(w, JPEG_QUALITY);
    img.write_with_encoder(encoder).map_err(io::Error::other)
}

fn encode_webp(w: &mut impl Write, img: &DynamicImage) -> io::Result<()> {
    let encoder = WebPEncoder::new_lossless(w);
    img.write_with_encoder(encoder).map_err(io::Error::other)
}

fn encode_gif(w: &mut impl Write, img: &DynamicImage) -> io::Result<()> {
    let encoder = GifEncoder::new(w);
    img.write_with_encoder(encoder).map_err(io::Error::other)
}

fn encode_bmp(w: &mut impl Write, img: &DynamicImage) -> io::Result<()> {
    let encoder = BmpEncoder::new(w);
    img.write_with_encoder(encoder).map_err(io::Error::other)
}

fn encode_tiff(w: &mut Cursor<Vec<u8>>, img: &DynamicImage) -> io::Result<()> {
    let encoder = TiffEncoder::new(w);
    img.write_with_encoder(encoder).map_err(io::Error::other)
}
