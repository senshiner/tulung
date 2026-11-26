//! Tool PDF untuk dokumen yang sudah ada: gabung, pisah, kompres,
//! putar, dan atur halaman.
//!
//! Diekspos sebagai fungsi `#[wasm_bindgen]` yang dipanggil shell web
//! (`web/gabung-pdf`, `web/pisah-pdf`, `web/kompres-pdf`,
//! `web/putar-pdf`, `web/atur-halaman`). Mesinnya lopdf — printpdf
//! hanya bisa membuat PDF baru, tidak memuat PDF yang sudah ada.
//! Semua pemrosesan jalan di browser, dokumen pengguna tidak pernah
//! diunggah ke server.
//!
//! Fungsi murni Rust (`merge_pdfs`, `split_pdf`, `rotate_pdf`,
//! `arrange_pages`, `compress_pdf`) terpisah dari pembungkus WASM
//! supaya bisa diuji tanpa browser.

use std::collections::HashMap;

use lopdf::{Dictionary, Document, Object, ObjectId};
use wasm_bindgen::prelude::*;

/// Muat byte PDF menjadi dokumen lopdf.
fn load(pdf: &[u8]) -> Result<Document, String> {
    Document::load_mem(pdf).map_err(|err| format!("File bukan PDF yang valid: {err}"))
}

/// Tulis dokumen lopdf menjadi byte PDF.
fn save(mut doc: Document) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|err| format!("Gagal menulis PDF hasil: {err}"))?;
    Ok(out)
}

/// Baca satu segmen pilihan halaman ("3" atau "2-5") menjadi daftar
/// nomor halaman 1-based. Rentang selalu menaik.
fn parse_segment(segment: &str, total: u32) -> Result<Vec<u32>, String> {
    let segment = segment.trim();
    if segment.is_empty() {
        return Err("Pilihan halaman ada yang kosong.".to_string());
    }
    let (start, end) = match segment.split_once('-') {
        Some((a, b)) => (
            a.trim()
                .parse::<u32>()
                .map_err(|_| format!("Nomor halaman tidak valid: \"{segment}\""))?,
            b.trim()
                .parse::<u32>()
                .map_err(|_| format!("Nomor halaman tidak valid: \"{segment}\""))?,
        ),
        None => {
            let n = segment
                .parse::<u32>()
                .map_err(|_| format!("Nomor halaman tidak valid: \"{segment}\""))?;
            (n, n)
        }
    };
    if start == 0 || end == 0 {
        return Err("Nomor halaman mulai dari 1.".to_string());
    }
    if end < start {
        return Err(format!(
            "Rentang halaman terbalik: \"{segment}\". Tulis dari kecil ke besar."
        ));
    }
    if end > total {
        return Err(format!(
            "Halaman {end} di luar dokumen ({total} halaman)."
        ));
    }
    Ok((start..=end).collect())
}

/// Baca pilihan halaman lengkap ("1-3,5") menjadi daftar nomor halaman
/// 1-based, urut seperti ditulis pengguna. String kosong berarti
/// semua halaman bila `empty_means_all`, selain itu error.
fn parse_page_numbers(spec: &str, total: u32, empty_means_all: bool) -> Result<Vec<u32>, String> {
    if spec.trim().is_empty() {
        if empty_means_all {
            return Ok((1..=total).collect());
        }
        return Err("Tulis nomor halaman dulu, mis. 3,1,2.".to_string());
    }
    let mut pages = Vec::new();
    for segment in spec.split(',') {
        pages.extend(parse_segment(segment, total)?);
    }
    Ok(pages)
}

/// Baca rentang pisah ("1-3,5") menjadi kelompok halaman, satu
/// kelompok per file hasil. String kosong berarti satu file per
/// halaman.
fn parse_ranges(spec: &str, total: u32) -> Result<Vec<Vec<u32>>, String> {
    if spec.trim().is_empty() {
        return Ok((1..=total).map(|page| vec![page]).collect());
    }
    spec.split(',').map(|s| parse_segment(s, total)).collect()
}

/// Kunci halaman yang bisa diwarisi dari simpul pohon halaman di
/// atasnya. Dokumen hasil ditulis dengan pohon halaman rata, jadi
/// nilai warisan harus disalin ke tiap halaman.
const INHERITABLE_KEYS: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Kamus halaman sumber dengan nilai warisan dari leluhurnya sudah
/// diterapkan dan kunci `Parent` dibuang (diganti saat impor).
fn page_with_inherited(src: &Document, page_id: ObjectId) -> Result<Dictionary, String> {
    let mut result = src
        .get_dictionary(page_id)
        .map_err(|err| err.to_string())?
        .clone();
    let mut parent = result
        .get(b"Parent")
        .and_then(|obj| obj.as_reference())
        .ok();
    while let Some(parent_id) = parent {
        let parent_dict = src
            .get_dictionary(parent_id)
            .map_err(|err| err.to_string())?;
        for key in INHERITABLE_KEYS {
            if !result.has(key) && parent_dict.has(key) {
                let value = parent_dict.get(key).map_err(|err| err.to_string())?;
                result.set(key.to_vec(), value.clone());
            }
        }
        parent = parent_dict
            .get(b"Parent")
            .and_then(|obj| obj.as_reference())
            .ok();
    }
    result.remove(b"Parent");
    Ok(result)
}

/// Penyalin objek antar-dokumen: menyalin graf objek yang dirujuk
/// satu halaman sumber ke dokumen hasil dengan nomor objek baru,
/// supaya halaman dari dokumen berbeda (atau pohon halaman bersarang)
/// bisa disusun dalam satu pohon halaman rata.
struct Importer<'a> {
    src: &'a Document,
    /// Objek sumber (selain halaman) yang sudah disalin.
    map: HashMap<ObjectId, ObjectId>,
    /// Halaman sumber -> objek hasil pertamanya. Halaman yang diminta
    /// dua kali disalin dari hasil pertama sebagai objek baru, jadi
    /// tiap entri pohon halaman tetap menunjuk objek yang berbeda.
    page_first: HashMap<ObjectId, ObjectId>,
}

impl<'a> Importer<'a> {
    fn new(src: &'a Document) -> Self {
        Self {
            src,
            map: HashMap::new(),
            page_first: HashMap::new(),
        }
    }

    fn import_object(
        &mut self,
        out: &mut Document,
        pages_root: ObjectId,
        obj: &Object,
    ) -> Result<Object, String> {
        match obj {
            Object::Reference(id) => Ok(Object::Reference(
                self.import_reference(out, pages_root, *id)?,
            )),
            Object::Array(items) => {
                let mut copied = Vec::with_capacity(items.len());
                for item in items {
                    copied.push(self.import_object(out, pages_root, item)?);
                }
                Ok(Object::Array(copied))
            }
            Object::Dictionary(dict) => Ok(Object::Dictionary(
                self.import_dict(out, pages_root, dict)?,
            )),
            Object::Stream(stream) => {
                let mut copied = stream.clone();
                copied.dict = self.import_dict(out, pages_root, &stream.dict)?;
                copied.start_position = None;
                Ok(Object::Stream(copied))
            }
            other => Ok(other.clone()),
        }
    }

    fn import_dict(
        &mut self,
        out: &mut Document,
        pages_root: ObjectId,
        dict: &Dictionary,
    ) -> Result<Dictionary, String> {
        let mut copied = Dictionary::new();
        for (key, value) in dict.iter() {
            let value = self.import_object(out, pages_root, value)?;
            copied.set(key.clone(), value);
        }
        Ok(copied)
    }

    fn import_reference(
        &mut self,
        out: &mut Document,
        pages_root: ObjectId,
        id: ObjectId,
    ) -> Result<ObjectId, String> {
        if let Some(new_id) = self.map.get(&id) {
            return Ok(*new_id);
        }
        let target = self
            .src
            .get_object(id)
            .map_err(|err| err.to_string())?
            .clone();
        // Halaman yang dirujuk objek lain (mis. anotasi) harus masuk
        // lewat jalur halaman supaya Parent-nya benar.
        let is_page = matches!(&target, Object::Dictionary(dict)
            if matches!(dict.get(b"Type"), Ok(Object::Name(name)) if name.as_slice() == b"Page"));
        if is_page {
            return self.import_page(out, pages_root, id);
        }
        let new_id = out.new_object_id();
        self.map.insert(id, new_id);
        let copied = self.import_object(out, pages_root, &target)?;
        out.objects.insert(new_id, copied);
        Ok(new_id)
    }

    fn import_page(
        &mut self,
        out: &mut Document,
        pages_root: ObjectId,
        page_id: ObjectId,
    ) -> Result<ObjectId, String> {
        if let Some(first) = self.page_first.get(&page_id) {
            let existing = out
                .objects
                .get(first)
                .cloned()
                .ok_or_else(|| "Halaman hasil impor tidak ditemukan.".to_string())?;
            let new_id = out.new_object_id();
            out.objects.insert(new_id, existing);
            return Ok(new_id);
        }
        let new_id = out.new_object_id();
        self.map.insert(page_id, new_id);
        self.page_first.insert(page_id, new_id);
        let dict = page_with_inherited(self.src, page_id)?;
        let mut copied = self.import_dict(out, pages_root, &dict)?;
        copied.set("Parent", Object::Reference(pages_root));
        out.objects.insert(new_id, Object::Dictionary(copied));
        Ok(new_id)
    }
}

/// Susun dokumen hasil dari daftar (dokumen sumber, nomor halaman
/// yang diambil berurutan). Pohon halaman hasil selalu rata: satu
/// simpul Pages berisi semua halaman.
fn build_document(sources: &[(&Document, Vec<u32>)]) -> Result<Vec<u8>, String> {
    let version = sources
        .first()
        .map(|(doc, _)| doc.version.clone())
        .unwrap_or_else(|| "1.5".to_string());
    let mut out = Document::with_version(version);
    let pages_root = out.new_object_id();
    let catalog_id = out.new_object_id();
    let mut kids: Vec<Object> = Vec::new();
    for (src, page_numbers) in sources {
        let pages = src.get_pages();
        let mut importer = Importer::new(src);
        for number in page_numbers {
            let page_id = pages
                .get(number)
                .ok_or_else(|| format!("Halaman {number} tidak ada di dokumen."))?;
            let new_id = importer.import_page(&mut out, pages_root, *page_id)?;
            kids.push(Object::Reference(new_id));
        }
    }
    if kids.is_empty() {
        return Err("Tidak ada halaman untuk ditulis.".to_string());
    }
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Count", Object::Integer(kids.len() as i64));
    pages_dict.set("Kids", Object::Array(kids));
    out.objects.insert(pages_root, Object::Dictionary(pages_dict));
    let mut catalog = Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_root));
    out.objects.insert(catalog_id, Object::Dictionary(catalog));
    out.trailer.set("Root", Object::Reference(catalog_id));
    save(out)
}

/// Gabung beberapa PDF menjadi satu PDF; urutan halaman mengikuti
/// urutan file masukan.
pub fn merge_pdfs(pdfs: &[&[u8]]) -> Result<Vec<u8>, String> {
    if pdfs.is_empty() {
        return Err("Tidak ada PDF untuk digabungkan.".to_string());
    }
    let mut docs = Vec::with_capacity(pdfs.len());
    for (index, bytes) in pdfs.iter().enumerate() {
        let doc = Document::load_mem(bytes).map_err(|err| {
            format!("File ke-{} bukan PDF yang valid: {err}", index + 1)
        })?;
        if doc.get_pages().is_empty() {
            return Err(format!("File ke-{} tidak punya halaman.", index + 1));
        }
        docs.push(doc);
    }
    let sources: Vec<(&Document, Vec<u32>)> = docs
        .iter()
        .map(|doc| {
            let total = doc.get_pages().len() as u32;
            (doc, (1..=total).collect::<Vec<u32>>())
        })
        .collect();
    build_document(&sources)
}

/// Pisah satu PDF menjadi beberapa PDF. `ranges` ditulis seperti
/// "1-3,5": tiap potongan menjadi satu file. String kosong memecah
/// semua halaman menjadi satu file per halaman.
pub fn split_pdf(pdf: &[u8], ranges: &str) -> Result<Vec<Vec<u8>>, String> {
    let doc = load(pdf)?;
    let total = doc.get_pages().len() as u32;
    if total == 0 {
        return Err("PDF tidak punya halaman.".to_string());
    }
    let groups = parse_ranges(ranges, total)?;
    let mut out = Vec::with_capacity(groups.len());
    for group in groups {
        out.push(build_document(&[(&doc, group)])?);
    }
    Ok(out)
}

/// Rotasi efektif satu halaman, termasuk nilai yang diwarisi dari
/// pohon halaman di atasnya.
fn effective_rotate(doc: &Document, page_id: ObjectId) -> i64 {
    let mut current = page_id;
    loop {
        let Ok(dict) = doc.get_dictionary(current) else {
            return 0;
        };
        if let Ok(value) = dict.get(b"Rotate").and_then(|obj| obj.as_i64()) {
            return value.rem_euclid(360);
        }
        match dict.get(b"Parent").and_then(|obj| obj.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return 0,
        }
    }
}

/// Putar halaman sebesar `degrees` (kelipatan 90, searah jarum jam).
/// `pages` kosong berarti semua halaman; selain itu ditulis seperti
/// "1-3,5".
pub fn rotate_pdf(pdf: &[u8], degrees: i32, pages: &str) -> Result<Vec<u8>, String> {
    if degrees % 90 != 0 {
        return Err(format!(
            "Rotasi harus kelipatan 90 derajat, bukan {degrees}."
        ));
    }
    let delta = degrees.rem_euclid(360) as i64;
    let mut doc = load(pdf)?;
    let total = doc.get_pages().len() as u32;
    if total == 0 {
        return Err("PDF tidak punya halaman.".to_string());
    }
    let targets = parse_page_numbers(pages, total, true)?;
    let page_map = doc.get_pages();
    for number in targets {
        let page_id = page_map[&number];
        let rotated = (effective_rotate(&doc, page_id) + delta) % 360;
        doc.get_dictionary_mut(page_id)
            .map_err(|err| err.to_string())?
            .set("Rotate", Object::Integer(rotated));
    }
    save(doc)
}

/// Susun ulang halaman sesuai `order` (mis. "3,1,2"): halaman hasil
/// tepat urutan itu; halaman yang tidak disebut dibuang. Nomor boleh
/// diulang, halaman akan muncul sebanyak disebut.
pub fn arrange_pages(pdf: &[u8], order: &str) -> Result<Vec<u8>, String> {
    let doc = load(pdf)?;
    let total = doc.get_pages().len() as u32;
    if total == 0 {
        return Err("PDF tidak punya halaman.".to_string());
    }
    let numbers = parse_page_numbers(order, total, false)?;
    build_document(&[(&doc, numbers)])
}

/// Sampel gambar yang bisa dikodekan ulang sebagai JPEG: RGB atau
/// abu-abu, 8 bit per komponen, tanpa topeng gambar.
fn image_samples(doc: &Document, id: ObjectId) -> Option<(u32, u32, usize, Vec<u8>)> {
    let object = doc.get_object(id).ok()?;
    let stream = match object {
        Object::Stream(stream) => stream,
        _ => return None,
    };
    let dict = &stream.dict;
    let is_image = matches!(dict.get(b"Type"), Ok(Object::Name(name)) if name.as_slice() == b"XObject")
        && matches!(dict.get(b"Subtype"), Ok(Object::Name(name)) if name.as_slice() == b"Image");
    if !is_image {
        return None;
    }
    if matches!(dict.get(b"ImageMask"), Ok(Object::Boolean(true))) {
        return None;
    }
    if !matches!(
        dict.get(b"BitsPerComponent").and_then(|obj| obj.as_i64()),
        Ok(8)
    ) {
        return None;
    }
    let width = dict.get(b"Width").and_then(|obj| obj.as_i64()).ok()?;
    let height = dict.get(b"Height").and_then(|obj| obj.as_i64()).ok()?;
    if width <= 0 || height <= 0 {
        return None;
    }
    let colorspace = match dict.get(b"ColorSpace") {
        Ok(obj) => match obj {
            Object::Reference(ref_id) => doc.get_object(*ref_id).ok()?,
            other => other,
        },
        Err(_) => return None,
    };
    let components = match colorspace {
        Object::Name(name) if name.as_slice() == b"DeviceRGB" => 3,
        Object::Name(name) if name.as_slice() == b"DeviceGray" => 1,
        Object::Array(items) => match items.first() {
            Some(Object::Name(name)) if name.as_slice() == b"DeviceRGB" => 3,
            Some(Object::Name(name)) if name.as_slice() == b"DeviceGray" => 1,
            _ => return None,
        },
        _ => return None,
    };
    let samples = stream.decompressed_content().ok()?;
    let expected = (width as usize) * (height as usize) * components;
    if samples.len() != expected {
        return None;
    }
    Some((width as u32, height as u32, components, samples))
}

/// Kodekan sampel RGB/abu-abu menjadi JPEG kualitas 60.
fn encode_jpeg(samples: &[u8], width: u32, height: u32, components: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 60);
    match components {
        3 => {
            let img = image::RgbImage::from_raw(width, height, samples.to_vec())
                .ok_or_else(|| "Ukuran sampel gambar tidak cocok.".to_string())?;
            img.write_with_encoder(encoder)
                .map_err(|err| err.to_string())?;
        }
        1 => {
            let img = image::GrayImage::from_raw(width, height, samples.to_vec())
                .ok_or_else(|| "Ukuran sampel gambar tidak cocok.".to_string())?;
            img.write_with_encoder(encoder)
                .map_err(|err| err.to_string())?;
        }
        _ => return Err("Format warna gambar tidak didukung.".to_string()),
    }
    Ok(out)
}

/// Kompres PDF: gambar RGB/abu-abu yang tersemat dikodekan ulang
/// sebagai JPEG kualitas 60 (hanya bila hasilnya lebih kecil dari
/// data yang tersimpan), lalu semua stream yang belum terkompresi
/// dimampatkan. Tata letak dan teks tidak berubah.
pub fn compress_pdf(pdf: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = load(pdf)?;
    let ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    let mut replacements: Vec<(ObjectId, usize, Vec<u8>)> = Vec::new();
    for id in ids {
        let Some((width, height, components, samples)) = image_samples(&doc, id) else {
            continue;
        };
        let stored_len = match doc.get_object(id) {
            Ok(Object::Stream(stream)) => stream.content.len(),
            _ => continue,
        };
        if let Ok(jpeg) = encode_jpeg(&samples, width, height, components) {
            if jpeg.len() < stored_len {
                replacements.push((id, components, jpeg));
            }
        }
    }
    for (id, components, jpeg) in replacements {
        if let Ok(Object::Stream(stream)) = doc.get_object_mut(id) {
            stream.dict.set(
                "ColorSpace",
                Object::Name(if components == 3 {
                    b"DeviceRGB".to_vec()
                } else {
                    b"DeviceGray".to_vec()
                }),
            );
            stream
                .dict
                .set("Filter", Object::Name(b"DCTDecode".to_vec()));
            stream.dict.remove(b"DecodeParms");
            stream.set_content(jpeg);
        }
    }
    doc.compress();
    save(doc)
}

/// Gabung beberapa PDF menjadi satu. Dipanggil shell web
/// (`web/gabung-pdf`) dengan array berisi byte tiap file.
#[wasm_bindgen]
pub fn gabung_pdf(pdfs: &js_sys::Array) -> Result<Vec<u8>, JsValue> {
    let mut decoded = Vec::with_capacity(pdfs.length() as usize);
    for value in pdfs.iter() {
        decoded.push(js_sys::Uint8Array::new(&value).to_vec());
    }
    let refs: Vec<&[u8]> = decoded.iter().map(|bytes| bytes.as_slice()).collect();
    merge_pdfs(&refs).map_err(|err| JsValue::from_str(&err))
}

/// Pisah satu PDF menjadi beberapa PDF. Dipanggil shell web
/// (`web/pisah-pdf`); hasil dikembalikan sebagai array byte PDF.
#[wasm_bindgen]
pub fn pisah_pdf(pdf: &[u8], ranges: &str) -> Result<js_sys::Array, JsValue> {
    let parts = split_pdf(pdf, ranges).map_err(|err| JsValue::from_str(&err))?;
    let out = js_sys::Array::new();
    for part in &parts {
        out.push(&js_sys::Uint8Array::from(part.as_slice()));
    }
    Ok(out)
}

/// Putar halaman PDF. Dipanggil shell web (`web/putar-pdf`).
#[wasm_bindgen]
pub fn putar_pdf(pdf: &[u8], degrees: i32, pages: &str) -> Result<Vec<u8>, JsValue> {
    rotate_pdf(pdf, degrees, pages).map_err(|err| JsValue::from_str(&err))
}

/// Atur ulang dan buang halaman PDF. Dipanggil shell web
/// (`web/atur-halaman`).
#[wasm_bindgen]
pub fn atur_halaman(pdf: &[u8], order: &str) -> Result<Vec<u8>, JsValue> {
    arrange_pages(pdf, order).map_err(|err| JsValue::from_str(&err))
}

/// Kompres satu PDF. Dipanggil shell web (`web/kompres-pdf`).
#[wasm_bindgen]
pub fn kompres_pdf(pdf: &[u8]) -> Result<Vec<u8>, JsValue> {
    compress_pdf(pdf).map_err(|err| JsValue::from_str(&err))
}
