//! Mesin Tulung gambar ke PDF: gabung gambar menjadi PDF,
//! dikompilasi ke WebAssembly.
//!
//! Diekspos sebagai fungsi `#[wasm_bindgen]` yang dipanggil
//! shell web (`web/gambar-ke-pdf`). Semua pemrosesan jalan di browser,
//! dokumen pengguna tidak pernah diunggah ke server.

use std::io::Cursor;

use image::ImageDecoder;
use printpdf::{
    ImageOptimizationOptions, Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Pt, RawImage,
    XObjectTransform,
};
use wasm_bindgen::prelude::*;

/// DPI acuan saat menempatkan gambar (sama dengan default printpdf).
const DPI: f32 = 300.0;

/// Opsi halaman untuk Gambar ke PDF, dipilih pengguna di halaman tool.
/// Default: potret, A4, tanpa margin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PdfOptions {
    pub orientation: Orientation,
    pub page_size: PageSize,
    pub margin: Margin,
}

/// Orientasi halaman.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Ukuran halaman: A4, US Letter, atau seukuran gambar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageSize {
    A4,
    Letter,
    Fit,
}

/// Margin halaman: tanpa margin, kecil (10 mm), besar (25 mm).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Margin {
    None,
    Small,
    Big,
}

impl PdfOptions {
    /// Opsi default: potret, A4, tanpa margin.
    pub fn default_options() -> Self {
        Self {
            orientation: Orientation::Portrait,
            page_size: PageSize::A4,
            margin: Margin::None,
        }
    }

    /// Baca opsi dari tiga string yang dikirim shell web.
    pub fn from_strings(
        orientation: &str,
        page_size: &str,
        margin: &str,
    ) -> Result<Self, String> {
        let orientation = match orientation {
            "portrait" => Orientation::Portrait,
            "landscape" => Orientation::Landscape,
            other => return Err(format!("Orientasi tidak dikenal: {other}")),
        };
        let page_size = match page_size {
            "a4" => PageSize::A4,
            "letter" => PageSize::Letter,
            "fit" => PageSize::Fit,
            other => return Err(format!("Ukuran halaman tidak dikenal: {other}")),
        };
        let margin = match margin {
            "none" => Margin::None,
            "small" => Margin::Small,
            "big" => Margin::Big,
            other => return Err(format!("Margin tidak dikenal: {other}")),
        };
        Ok(Self {
            orientation,
            page_size,
            margin,
        })
    }
}

/// Lebar dan tinggi halaman dalam mm untuk A4 dan US Letter.
fn page_size_mm(size: PageSize, orientation: Orientation) -> (f32, f32) {
    let (w, h) = match size {
        PageSize::A4 => (210.0, 297.0),
        PageSize::Letter => (215.9, 279.4),
        PageSize::Fit => unreachable!("ukuran Fit dihitung per gambar"),
    };
    match orientation {
        Orientation::Portrait => (w, h),
        Orientation::Landscape => (h, w),
    }
}

/// Margin dalam mm.
fn margin_mm(margin: Margin) -> f32 {
    match margin {
        Margin::None => 0.0,
        Margin::Small => 10.0,
        Margin::Big => 25.0,
    }
}

/// Gabung beberapa gambar menjadi satu PDF, satu gambar satu halaman,
/// dengan opsi default (potret, A4, tanpa margin).
pub fn build_pdf_from_images(images: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    build_pdf_from_images_with_options(images, &PdfOptions::default_options())
}

/// Gabung beberapa gambar menjadi satu PDF, satu gambar satu halaman,
/// dengan opsi halaman yang dipilih pengguna.
///
/// Setiap gambar mengisi halaman: diskalakan sampai mentok lebar atau
/// tinggi area (boleh diperbesar), tetap menjaga rasio, dan berada di
/// tengah halaman. Resolusi gambar tidak dibatasi: batas 2 MB bawaan
/// printpdf dimatikan supaya foto besar tidak diperkecil diam-diam.
pub fn build_pdf_from_images_with_options(
    images: &[Vec<u8>],
    options: &PdfOptions,
) -> Result<Vec<u8>, String> {
    let rotations = vec![0; images.len()];
    build_pdf_from_images_with_rotations(images, &rotations, options)
}

/// Seperti `build_pdf_from_images_with_options`, plus rotasi manual
/// per gambar (derajat, kelipatan 90, searah jarum jam). Orientasi
/// EXIF selalu diterapkan lebih dulu, jadi hasil PDF sama dengan
/// yang terlihat di pratinjau browser.
pub fn build_pdf_from_images_with_rotations(
    images: &[Vec<u8>],
    rotations: &[u32],
    options: &PdfOptions,
) -> Result<Vec<u8>, String> {
    if images.is_empty() {
        return Err("Tidak ada gambar untuk diubah menjadi PDF.".to_string());
    }
    let mut doc = PdfDocument::new("Tulung");
    let mut pages = Vec::with_capacity(images.len());
    let mut warnings = Vec::new();
    for (index, bytes) in images.iter().enumerate() {
        let rotation = rotations.get(index).copied().unwrap_or(0);
        let prepared = prepare_image(bytes, rotation)
            .map_err(|err| format!("Gambar ke-{} tidak bisa dibaca: {err}", index + 1))?;
        let image = RawImage::decode_from_bytes(&prepared, &mut warnings)
            .map_err(|err| format!("Gambar ke-{} tidak bisa dibaca: {err}", index + 1))?;
        let image_id = doc.add_image(&image);
        let (page_w_mm, page_h_mm, transform) = page_layout(&image, options);
        let ops = vec![Op::UseXobject {
            id: image_id,
            transform,
        }];
        pages.push(PdfPage::new(Mm(page_w_mm), Mm(page_h_mm), ops));
    }
    let save_options = PdfSaveOptions {
        image_optimization: Some(ImageOptimizationOptions {
            max_image_size: None,
            ..Default::default()
        }),
        ..Default::default()
    };
    Ok(doc
        .with_pages(pages)
        .save(&save_options, &mut warnings))
}

/// Siapkan byte gambar sebelum disematkan ke PDF: terapkan orientasi
/// EXIF, lalu rotasi manual dari pengguna.
///
/// Foto dari HP sering disimpan miring (mis. lanskap) dengan penanda
/// EXIF yang menyatakan cara menampilkannya (potret). Browser
/// menghormati penanda itu di pratinjau, tapi byte mentahnya tetap
/// miring — tanpa langkah ini hasil PDF jadi menyamping. Gambar yang
/// tidak perlu diubah dikembalikan apa adanya, supaya kualitasnya
/// tidak turun karena encode ulang.
fn prepare_image(bytes: &[u8], rotation: u32) -> Result<Vec<u8>, String> {
    let rotation = rotation % 360;
    if rotation % 90 != 0 {
        return Err(format!(
            "Rotasi harus kelipatan 90 derajat, bukan {rotation}."
        ));
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes));
    reader = reader
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let format = reader.format();
    let mut decoder = reader.into_decoder().map_err(|err| err.to_string())?;
    let orientation = decoder.orientation().map_err(|err| err.to_string())?;
    let has_exif_orientation = orientation != image::metadata::Orientation::NoTransforms;
    if !has_exif_orientation && rotation == 0 {
        return Ok(bytes.to_vec());
    }
    let mut img =
        image::DynamicImage::from_decoder(decoder).map_err(|err| err.to_string())?;
    if has_exif_orientation {
        img.apply_orientation(orientation);
    }
    img = match rotation {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    };
    let mut out = Vec::new();
    match format {
        Some(image::ImageFormat::Jpeg) => {
            let encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
            img.write_with_encoder(encoder)
                .map_err(|err| err.to_string())?;
        }
        _ => {
            img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(out)
}

/// Hitung ukuran halaman dan transform gambar.
///
/// PDF memakai titik (pt): 1 inci = 72 pt. Ukuran gambar dalam pt
/// pada DPI acuan = piksel * 72 / DPI. Untuk ukuran "Sesuai gambar",
/// halaman seukuran gambar dan margin diabaikan.
fn page_layout(image: &RawImage, options: &PdfOptions) -> (f32, f32, XObjectTransform) {
    let (page_w_mm, page_h_mm, margin_pt) = match options.page_size {
        PageSize::Fit => (
            image.width as f32 * 25.4 / DPI,
            image.height as f32 * 25.4 / DPI,
            0.0,
        ),
        size => {
            let (w, h) = page_size_mm(size, options.orientation);
            (w, h, margin_mm(options.margin) / 25.4 * 72.0)
        }
    };
    let page_w_pt = page_w_mm / 25.4 * 72.0;
    let page_h_pt = page_h_mm / 25.4 * 72.0;
    let avail_w = page_w_pt - 2.0 * margin_pt;
    let avail_h = page_h_pt - 2.0 * margin_pt;
    let img_w_pt = image.width as f32 * 72.0 / DPI;
    let img_h_pt = image.height as f32 * 72.0 / DPI;
    let scale = (avail_w / img_w_pt).min(avail_h / img_h_pt);
    let draw_w = img_w_pt * scale;
    let draw_h = img_h_pt * scale;
    let transform = XObjectTransform {
        translate_x: Some(Pt(margin_pt + (avail_w - draw_w) / 2.0)),
        translate_y: Some(Pt(margin_pt + (avail_h - draw_h) / 2.0)),
        rotate: None,
        scale_x: Some(scale),
        scale_y: Some(scale),
        dpi: Some(DPI),
    };
    (page_w_mm, page_h_mm, transform)
}

/// Gambar ke PDF, dipanggil shell web (`web/gambar-ke-pdf/app.js`).
///
/// Menerima array `Uint8Array` (satu per gambar), array rotasi manual
/// per gambar (derajat, kelipatan 90), plus tiga string opsi
/// (orientasi, ukuran halaman, margin) dan mengembalikan byte PDF
/// sebagai `Uint8Array`.
#[wasm_bindgen]
pub fn images_to_pdf(
    images: &js_sys::Array,
    rotations: &js_sys::Array,
    orientation: &str,
    page_size: &str,
    margin: &str,
) -> Result<Vec<u8>, JsValue> {
    let options = PdfOptions::from_strings(orientation, page_size, margin)
        .map_err(|err| JsValue::from_str(&err))?;
    let mut decoded = Vec::with_capacity(images.length() as usize);
    for value in images.iter() {
        decoded.push(js_sys::Uint8Array::new(&value).to_vec());
    }
    let mut rots = Vec::with_capacity(images.length() as usize);
    for value in rotations.iter() {
        rots.push(value.as_f64().unwrap_or(0.0) as u32);
    }
    build_pdf_from_images_with_rotations(&decoded, &rots, &options)
        .map_err(|err| JsValue::from_str(&err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::new(width, height);
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    fn page_count(pdf: &[u8]) -> usize {
        let text = String::from_utf8_lossy(pdf);
        // "/Type/Page/" hanya cocok dengan objek halaman, bukan "/Type/Pages".
        text.matches("/Type/Page/").count()
    }

    /// Ukuran gambar yang digambar (lebar x tinggi, pt) per halaman.
    fn draw_sizes(pdf: &[u8]) -> Vec<(f32, f32)> {
        let text = String::from_utf8_lossy(pdf);
        let mut sizes = Vec::new();
        let mut rest = text.as_ref();
        while let Some(pos) = rest.find(" 0 0 ") {
            let head = &rest[..pos];
            let tail = &rest[pos + 5..];
            let w: f32 = head
                .split_whitespace()
                .last()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            let mut parts = tail.split_whitespace();
            let h: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
            let line_end = tail.find('\n').unwrap_or(tail.len());
            if tail[..line_end].contains(" cm") {
                sizes.push((w, h));
            }
            rest = tail;
        }
        sizes
    }

    /// Ukuran halaman pertama (lebar x tinggi, pt) dari /MediaBox.
    fn first_page_size(pdf: &[u8]) -> (f32, f32) {
        let text = String::from_utf8_lossy(pdf);
        let start = text.find("/MediaBox[0 0 ").expect("MediaBox tidak ada");
        let rest = &text[start + 14..];
        let mut parts = rest.split(|c: char| c == ' ' || c == ']');
        let w: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
        let h: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
        (w, h)
    }

    /// Lebar gambar terbesar yang disematkan (px).
    fn max_embedded_width(pdf: &[u8]) -> u32 {
        let text = String::from_utf8_lossy(pdf);
        let mut max = 0;
        let mut rest = text.as_ref();
        while let Some(pos) = rest.find("/Width ") {
            let tail = &rest[pos + 7..];
            // Angka lebar langsung ditempel "/Height" (tanpa spasi).
            let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
            let w: u32 = digits.parse().unwrap_or(0);
            if w > max {
                max = w;
            }
            rest = tail;
        }
        max
    }

    fn options(orientation: &str, page_size: &str, margin: &str) -> PdfOptions {
        PdfOptions::from_strings(orientation, page_size, margin).unwrap()
    }

    #[test]
    fn pdf_dari_satu_gambar() {
        let pdf = build_pdf_from_images(&[test_png(800, 600)]).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn pdf_dari_tiga_gambar_punya_tiga_halaman() {
        let images = vec![test_png(800, 600), test_png(600, 800), test_png(400, 400)];
        let pdf = build_pdf_from_images(&images).unwrap();
        assert_eq!(page_count(&pdf), 3);
    }

    #[test]
    fn daftar_kosong_ditolak() {
        assert!(build_pdf_from_images(&[]).is_err());
    }

    #[test]
    fn byte_rusak_ditolak() {
        let junk = b"bukan gambar sama sekali".to_vec();
        assert!(build_pdf_from_images(&[junk]).is_err());
    }

    #[test]
    fn opsi_tidak_dikenal_ditolak() {
        assert!(PdfOptions::from_strings("miring", "a4", "none").is_err());
        assert!(PdfOptions::from_strings("portrait", "a3", "none").is_err());
        assert!(PdfOptions::from_strings("portrait", "a4", "sedang").is_err());
    }

    #[test]
    fn gambar_kecil_mengisi_lebar_halaman() {
        // Default (potret, A4, tanpa margin): 400x300 px mentok lebar 595 pt.
        let pdf = build_pdf_from_images(&[test_png(400, 300)]).unwrap();
        let (w, h) = draw_sizes(&pdf)[0];
        assert!((w - 595.28).abs() < 0.5, "lebar gambar {w}");
        assert!((h - 446.46).abs() < 0.5, "tinggi gambar {h}");
    }

    #[test]
    fn gambar_tinggi_mengisi_tinggi_halaman() {
        // 800x2400 px mentok tinggi 842 pt, di tengah secara horizontal.
        let pdf = build_pdf_from_images(&[test_png(800, 2400)]).unwrap();
        let (w, h) = draw_sizes(&pdf)[0];
        assert!((w - 280.66).abs() < 0.5, "lebar gambar {w}");
        assert!((h - 841.89).abs() < 0.5, "tinggi gambar {h}");
    }

    #[test]
    fn orientasi_lanskap_membalik_halaman() {
        let opts = options("landscape", "a4", "none");
        let pdf = build_pdf_from_images_with_options(&[test_png(400, 300)], &opts).unwrap();
        let (w, h) = first_page_size(&pdf);
        assert!((w - 841.89).abs() < 0.5, "lebar halaman {w}");
        assert!((h - 595.28).abs() < 0.5, "tinggi halaman {h}");
    }

    #[test]
    fn ukuran_letter() {
        let opts = options("portrait", "letter", "none");
        let pdf = build_pdf_from_images_with_options(&[test_png(400, 300)], &opts).unwrap();
        let (w, h) = first_page_size(&pdf);
        assert!((w - 612.0).abs() < 0.5, "lebar halaman {w}");
        assert!((h - 792.0).abs() < 0.5, "tinggi halaman {h}");
    }

    #[test]
    fn ukuran_sesuai_gambar() {
        // Halaman seukuran gambar: 400x300 px = 96x72 pt, margin diabaikan.
        let opts = options("portrait", "fit", "big");
        let pdf = build_pdf_from_images_with_options(&[test_png(400, 300)], &opts).unwrap();
        let (w, h) = first_page_size(&pdf);
        assert!((w - 96.0).abs() < 0.5, "lebar halaman {w}");
        assert!((h - 72.0).abs() < 0.5, "tinggi halaman {h}");
    }

    #[test]
    fn margin_besar_mengecilkan_area() {
        // Margin 25 mm: lebar area 595,28 - 2x70,87 = 453,54 pt.
        let opts = options("portrait", "a4", "big");
        let pdf = build_pdf_from_images_with_options(&[test_png(400, 300)], &opts).unwrap();
        let (w, _) = draw_sizes(&pdf)[0];
        assert!((w - 453.54).abs() < 0.5, "lebar gambar {w}");
    }

    #[test]
    fn gambar_besar_tidak_dikecilkan_resolusinya() {
        // 2400x1800 px disematkan utuh (batas 2 MB bawaan dimatikan).
        let pdf = build_pdf_from_images(&[test_png(2400, 1800)]).unwrap();
        assert_eq!(max_embedded_width(&pdf), 2400);
    }

    #[test]
    fn orientasi_exif_diterapkan() {
        // Fixture: JPEG tersimpan 600x300 (lanskap) dengan EXIF
        // orientation=6; tampilan yang benar 300x600 (potret).
        // Ukuran Fit memakai ukuran tampil: 300 px -> 72 pt,
        // 600 px -> 144 pt pada 300 DPI.
        let bytes = include_bytes!("../tests/fixtures/exif-orientasi-6.jpg");
        let opts = options("portrait", "fit", "none");
        let pdf = build_pdf_from_images_with_options(&[bytes.to_vec()], &opts).unwrap();
        let (w, h) = first_page_size(&pdf);
        assert!((w - 72.0).abs() < 0.5, "lebar halaman {w}");
        assert!((h - 144.0).abs() < 0.5, "tinggi halaman {h}");
    }

    #[test]
    fn rotasi_90_menukar_dimensi() {
        // 400x300 diputar 90 derajat -> tampil 300x400 px.
        // Ukuran Fit: 300 px -> 72 pt, 400 px -> 96 pt.
        let opts = options("portrait", "fit", "none");
        let pdf =
            build_pdf_from_images_with_rotations(&[test_png(400, 300)], &[90], &opts)
                .unwrap();
        let (w, h) = first_page_size(&pdf);
        assert!((w - 72.0).abs() < 0.5, "lebar halaman {w}");
        assert!((h - 96.0).abs() < 0.5, "tinggi halaman {h}");
    }

    #[test]
    fn rotasi_bukan_kelipatan_90_ditolak() {
        let opts = PdfOptions::default_options();
        assert!(
            build_pdf_from_images_with_rotations(&[test_png(10, 10)], &[45], &opts)
                .is_err()
        );
    }
}
