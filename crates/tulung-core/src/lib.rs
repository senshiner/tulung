//! Mesin Tulung: logika semua tools, dikompilasi ke WebAssembly.
//!
//! Tiap tool diekspos sebagai fungsi `#[wasm_bindgen]` yang dipanggil
//! shell web (`web/`). Semua pemrosesan jalan di browser, dokumen
//! pengguna tidak pernah diunggah ke server.

use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Pt, RawImage, XObjectTransform};
use wasm_bindgen::prelude::*;

/// Versi tulung-core yang sedang berjalan di browser.
#[wasm_bindgen]
pub fn tulung_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Ukuran halaman A4 dalam mm.
const PAGE_WIDTH_MM: f32 = 210.0;
/// Tinggi halaman A4 dalam mm.
const PAGE_HEIGHT_MM: f32 = 297.0;
/// Margin halaman dalam mm.
const MARGIN_MM: f32 = 10.0;
/// DPI acuan saat menempatkan gambar (sama dengan default printpdf).
const DPI: f32 = 300.0;

/// Gabung beberapa gambar menjadi satu PDF, satu gambar satu halaman A4.
///
/// Setiap gambar dimuat di dalam margin, tetap menjaga rasio, dan
/// berada di tengah halaman. Ini memperbaiki dua bug my-pdf:
/// gambar tinggi tidak lagi meluber keluar halaman, dan tidak ada
/// lagi endpoint backend yang bisa salah alamat, karena semua proses
/// jalan di browser.
pub fn build_pdf_from_images(images: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if images.is_empty() {
        return Err("Tidak ada gambar untuk diubah menjadi PDF.".to_string());
    }
    let mut doc = PdfDocument::new("Tulung");
    let mut pages = Vec::with_capacity(images.len());
    let mut warnings = Vec::new();
    for (index, bytes) in images.iter().enumerate() {
        let image = RawImage::decode_from_bytes(bytes, &mut warnings)
            .map_err(|err| format!("Gambar ke-{} tidak bisa dibaca: {err}", index + 1))?;
        let image_id = doc.add_image(&image);
        let ops = vec![Op::UseXobject {
            id: image_id,
            transform: fit_transform(&image),
        }];
        pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));
    }
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings))
}

/// Hitung transform agar gambar muat di halaman A4 dengan margin,
/// tetap menjaga rasio, dan berada di tengah halaman.
///
/// PDF memakai titik (pt): 1 inci = 72 pt. Ukuran gambar dalam pt
/// pada DPI acuan = piksel * 72 / DPI. Gambar kecil tidak diperbesar
/// (skala dibatasi 1.0) supaya tetap tajam.
fn fit_transform(image: &RawImage) -> XObjectTransform {
    let page_w_pt = PAGE_WIDTH_MM / 25.4 * 72.0;
    let page_h_pt = PAGE_HEIGHT_MM / 25.4 * 72.0;
    let margin_pt = MARGIN_MM / 25.4 * 72.0;
    let avail_w = page_w_pt - 2.0 * margin_pt;
    let avail_h = page_h_pt - 2.0 * margin_pt;
    let img_w_pt = image.width as f32 * 72.0 / DPI;
    let img_h_pt = image.height as f32 * 72.0 / DPI;
    let scale = (avail_w / img_w_pt).min(avail_h / img_h_pt).min(1.0);
    let draw_w = img_w_pt * scale;
    let draw_h = img_h_pt * scale;
    XObjectTransform {
        translate_x: Some(Pt((page_w_pt - draw_w) / 2.0)),
        translate_y: Some(Pt((page_h_pt - draw_h) / 2.0)),
        rotate: None,
        scale_x: Some(scale),
        scale_y: Some(scale),
        dpi: Some(DPI),
    }
}

/// Gambar ke PDF, dipanggil shell web (`web/gambar-ke-pdf/app.js`).
///
/// Menerima array `Uint8Array` (satu per gambar) dan mengembalikan
/// byte PDF sebagai `Uint8Array`.
#[wasm_bindgen]
pub fn images_to_pdf(images: &js_sys::Array) -> Result<Vec<u8>, JsValue> {
    let mut decoded = Vec::with_capacity(images.length() as usize);
    for value in images.iter() {
        decoded.push(js_sys::Uint8Array::new(&value).to_vec());
    }
    build_pdf_from_images(&decoded).map_err(|err| JsValue::from_str(&err))
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
    fn gambar_tinggi_tidak_meluber() {
        // 1000x4000 px: tinggi alami 960 pt, halaman A4 hanya muat 785 pt.
        let mut warnings = Vec::new();
        let image =
            RawImage::decode_from_bytes(&test_png(1000, 4000), &mut warnings).unwrap();
        let transform = fit_transform(&image);
        let scale = transform.scale_x.unwrap();
        let img_h_pt = 4000.0 * 72.0 / DPI;
        let draw_h = img_h_pt * scale;
        let avail_h = (PAGE_HEIGHT_MM - 2.0 * MARGIN_MM) / 25.4 * 72.0;
        assert!(scale < 1.0);
        assert!((draw_h - avail_h).abs() < 0.01);
    }
}

