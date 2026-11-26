//! Tes tool PDF (gabung, pisah, putar, atur halaman, kompres).
//!
//! PDF uji dibuat dengan printpdf: tiap halaman punya lebar berbeda
//! dalam mm, jadi urutan halaman hasil bisa diverifikasi dari
//! MediaBox lewat lopdf.

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use printpdf::{Mm, PdfDocument, PdfPage, PdfSaveOptions};
use tulung_pdf::ops::{arrange_pages, compress_pdf, merge_pdfs, rotate_pdf, split_pdf};

fn sample_pdf(widths_mm: &[f32]) -> Vec<u8> {
    let mut doc = PdfDocument::new("uji");
    let pages: Vec<PdfPage> = widths_mm
        .iter()
        .map(|w| PdfPage::new(Mm(*w), Mm(200.0), Vec::new()))
        .collect();
    doc.with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new())
}

fn number(obj: &Object) -> f64 {
    match obj {
        Object::Integer(i) => *i as f64,
        Object::Real(r) => *r as f64,
        other => panic!("bukan angka: {other:?}"),
    }
}

fn page_count(pdf: &[u8]) -> usize {
    Document::load_mem(pdf).unwrap().get_pages().len()
}

/// Lebar tiap halaman dalam mm, berurutan.
fn page_widths_mm(pdf: &[u8]) -> Vec<f64> {
    let doc = Document::load_mem(pdf).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            let dict = doc.get_dictionary(*id).unwrap();
            let media_box = dict.get(b"MediaBox").unwrap().as_array().unwrap();
            (number(&media_box[2]) - number(&media_box[0])) * 25.4 / 72.0
        })
        .collect()
}

fn assert_widths(pdf: &[u8], expected_mm: &[f64]) {
    let widths = page_widths_mm(pdf);
    assert_eq!(widths.len(), expected_mm.len(), "jumlah halaman beda");
    for (got, want) in widths.iter().zip(expected_mm) {
        assert!((got - want).abs() < 1.0, "lebar {got} != {want}");
    }
}

/// Nilai Rotate tiap halaman, berurutan (0 bila tidak ada).
fn page_rotations(pdf: &[u8]) -> Vec<i64> {
    let doc = Document::load_mem(pdf).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            doc.get_dictionary(*id)
                .unwrap()
                .get(b"Rotate")
                .and_then(|obj| obj.as_i64())
                .unwrap_or(0)
        })
        .collect()
}

#[test]
fn gabung_menyatukan_halaman_sesuai_urutan_file() {
    let a = sample_pdf(&[100.0, 110.0]);
    let b = sample_pdf(&[120.0]);
    let merged = merge_pdfs(&[&a, &b]).unwrap();
    assert_eq!(page_count(&merged), 3);
    assert_widths(&merged, &[100.0, 110.0, 120.0]);
}

#[test]
fn gabung_satu_file_tetap_jalan() {
    let a = sample_pdf(&[100.0, 110.0]);
    let merged = merge_pdfs(&[&a]).unwrap();
    assert_widths(&merged, &[100.0, 110.0]);
}

#[test]
fn gabung_menolak_file_bukan_pdf() {
    let a = sample_pdf(&[100.0]);
    let err = merge_pdfs(&[&a, b"bukan pdf"]).unwrap_err();
    assert!(err.contains("File ke-2"), "{err}");
}

#[test]
fn gabung_tanpa_file_ditolak() {
    assert!(merge_pdfs(&[]).is_err());
}

#[test]
fn pisah_rentang_jadi_satu_file_per_potongan() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0, 130.0]);
    let parts = split_pdf(&pdf, "1-2,4").unwrap();
    assert_eq!(parts.len(), 2);
    assert_widths(&parts[0], &[100.0, 110.0]);
    assert_widths(&parts[1], &[130.0]);
}

#[test]
fn pisah_kosong_jadi_satu_file_per_halaman() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0]);
    let parts = split_pdf(&pdf, "").unwrap();
    assert_eq!(parts.len(), 3);
    assert_widths(&parts[0], &[100.0]);
    assert_widths(&parts[1], &[110.0]);
    assert_widths(&parts[2], &[120.0]);
}

#[test]
fn pisah_menolak_halaman_di_luar_dokumen() {
    let pdf = sample_pdf(&[100.0, 110.0]);
    assert!(split_pdf(&pdf, "3").is_err());
    assert!(split_pdf(&pdf, "2-1").is_err());
    assert!(split_pdf(&pdf, "x").is_err());
}

#[test]
fn putar_semua_halaman() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0]);
    let rotated = rotate_pdf(&pdf, 90, "").unwrap();
    assert_eq!(page_rotations(&rotated), vec![90, 90, 90]);
    assert_widths(&rotated, &[100.0, 110.0, 120.0]);
}

#[test]
fn putar_rentang_tertentu_dan_menumpuk() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0]);
    let rotated = rotate_pdf(&pdf, 90, "").unwrap();
    let rotated = rotate_pdf(&rotated, 180, "2").unwrap();
    assert_eq!(page_rotations(&rotated), vec![90, 270, 90]);
}

#[test]
fn putar_derajat_bukan_kelipatan_90_ditolak() {
    let pdf = sample_pdf(&[100.0]);
    assert!(rotate_pdf(&pdf, 45, "").is_err());
}

#[test]
fn putar_negatif_dinormalkan() {
    let pdf = sample_pdf(&[100.0]);
    let rotated = rotate_pdf(&pdf, -90, "").unwrap();
    assert_eq!(page_rotations(&rotated), vec![270]);
}

#[test]
fn atur_urutan_halaman() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0]);
    let arranged = arrange_pages(&pdf, "3,1,2").unwrap();
    assert_widths(&arranged, &[120.0, 100.0, 110.0]);
}

#[test]
fn atur_halaman_membuang_yang_tidak_disebut() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0]);
    let arranged = arrange_pages(&pdf, "2").unwrap();
    assert_widths(&arranged, &[110.0]);
}

#[test]
fn atur_halaman_dengan_rentang_dan_duplikat() {
    let pdf = sample_pdf(&[100.0, 110.0, 120.0]);
    let arranged = arrange_pages(&pdf, "2-3,2").unwrap();
    assert_eq!(page_count(&arranged), 3);
    assert_widths(&arranged, &[110.0, 120.0, 110.0]);
}

#[test]
fn atur_tanpa_urutan_ditolak() {
    let pdf = sample_pdf(&[100.0]);
    assert!(arrange_pages(&pdf, "").is_err());
    assert!(arrange_pages(&pdf, "5").is_err());
}

/// PDF satu halaman berisi gambar RGB 200x200 piksel acak yang
/// disimpan mentah (tanpa filter), dibuat langsung dengan lopdf.
fn pdf_with_raw_image() -> Vec<u8> {
    let (width, height) = (200u32, 200u32);
    let mut samples = Vec::with_capacity((width * height * 3) as usize);
    let mut state = 0x1234_5678u32;
    for _ in 0..width * height * 3 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        samples.push((state >> 24) as u8);
    }
    let mut doc = Document::with_version("1.5");
    let mut image_dict = Dictionary::new();
    image_dict.set("Type", Object::Name(b"XObject".to_vec()));
    image_dict.set("Subtype", Object::Name(b"Image".to_vec()));
    image_dict.set("Width", Object::Integer(width as i64));
    image_dict.set("Height", Object::Integer(height as i64));
    image_dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
    image_dict.set("BitsPerComponent", Object::Integer(8));
    let image_id: ObjectId = doc.add_object(Object::Stream(Stream::new(image_dict, samples)));
    let content_id: ObjectId = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        b"q 200 0 0 200 0 0 cm /Im1 Do Q".to_vec(),
    )));
    let mut xobjects = Dictionary::new();
    xobjects.set("Im1", Object::Reference(image_id));
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));
    let resources_id: ObjectId = doc.add_object(Object::Dictionary(resources));
    let pages_id = doc.new_object_id();
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_id));
    page.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(200),
            Object::Integer(200),
        ]),
    );
    page.set("Resources", Object::Reference(resources_id));
    page.set("Contents", Object::Reference(content_id));
    let page_id: ObjectId = doc.add_object(Object::Dictionary(page));
    let mut pages = Dictionary::new();
    pages.set("Type", Object::Name(b"Pages".to_vec()));
    pages.set("Count", Object::Integer(1));
    pages.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_id));
    let catalog_id: ObjectId = doc.add_object(Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn kompres_memperkecil_pdf_bergambar_mentah() {
    let pdf = pdf_with_raw_image();
    let compressed = compress_pdf(&pdf).unwrap();
    println!(
        "kompres gambar mentah: {} -> {} byte",
        pdf.len(),
        compressed.len()
    );
    assert_eq!(page_count(&compressed), 1);
    assert!(
        compressed.len() < pdf.len(),
        "hasil kompres {} tidak lebih kecil dari asal {}",
        compressed.len(),
        pdf.len()
    );
}

#[test]
fn kompres_pdf_biasa_tetap_valid() {
    let pdf = sample_pdf(&[100.0, 110.0]);
    let compressed = compress_pdf(&pdf).unwrap();
    assert_eq!(page_count(&compressed), 2);
    assert_widths(&compressed, &[100.0, 110.0]);
}

#[test]
fn kompres_menolak_file_bukan_pdf() {
    assert!(compress_pdf(b"bukan pdf").is_err());
}
