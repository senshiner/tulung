//! Mesin Tulung Doc Format: pindai dan perbaiki format .docx,
//! dikompilasi ke WebAssembly.
//!
//! Diekspos sebagai fungsi `#[wasm_bindgen]` yang dipanggil
//! shell web (`web/doc-format`). Semua pemrosesan jalan di browser,
//! dokumen pengguna tidak pernah diunggah ke server.

use wasm_bindgen::prelude::*;

mod docx;

/// Doc Format: pindai .docx terhadap preset, kembalikan laporan
/// sebagai JSON string (dipanggil shell web `web/doc-format/app.js`).
#[wasm_bindgen]
pub fn scan_docx(bytes: &[u8], preset: &str) -> Result<String, JsValue> {
    docx::scan_docx(bytes, preset).map_err(|err| JsValue::from_str(&err))
}

/// Doc Format: seluruh teks dokumen sebagai string, untuk mode
/// banding sebelum/sesudah (memastikan teks isi tidak berubah).
/// Daftar isi, tabel, dan gambar dibangun ulang bila ada.
#[wasm_bindgen]
pub fn docx_text(bytes: &[u8]) -> Result<String, JsValue> {
    docx::docx_text(bytes).map_err(|err| JsValue::from_str(&err))
}

/// Doc Format: profil bawaan (Standar Umum) sebagai JSON, dipakai
/// formulir profil kustom untuk mengisi kolom dengan angka default.
#[wasm_bindgen]
pub fn default_profile() -> Result<String, JsValue> {
    serde_json::to_string(&docx::builtin_profile()).map_err(|err| JsValue::from_str(&err.to_string()))
}

/// Doc Format: profil sebuah preset ("umum" / "makalah") sebagai
/// JSON, dipakai pemilih preset di shell untuk mengisi formulir.
#[wasm_bindgen]
pub fn preset_profile(preset: &str) -> Result<String, JsValue> {
    let profile = docx::preset(preset).map_err(|err| JsValue::from_str(&err))?;
    serde_json::to_string(&profile).map_err(|err| JsValue::from_str(&err.to_string()))
}

/// Doc Format: pindai .docx terhadap profil kustom (JSON), kembalikan
/// laporan sebagai JSON string.
#[wasm_bindgen]
pub fn scan_docx_profile(bytes: &[u8], profile_json: &str) -> Result<String, JsValue> {
    docx::scan_docx_profile(bytes, profile_json).map_err(|err| JsValue::from_str(&err))
}

/// Doc Format: daftar paragraf isi dokumen sebagai JSON, dipakai
/// pemilih batas penomoran halaman di shell web.
#[wasm_bindgen]
pub fn docx_paragraphs(bytes: &[u8]) -> Result<String, JsValue> {
    docx::docx_paragraphs(bytes).map_err(|err| JsValue::from_str(&err))
}

/// Doc Format: perbaiki .docx sesuai profil kustom (JSON), kembalikan
/// byte .docx.
#[wasm_bindgen]
pub fn fix_docx_profile(bytes: &[u8], profile_json: &str) -> Result<Vec<u8>, JsValue> {
    docx::fix_docx_profile(bytes, profile_json).map_err(|err| JsValue::from_str(&err))
}

/// Doc Format: perbaiki .docx sesuai preset, kembalikan byte .docx.
#[wasm_bindgen]
pub fn fix_docx(bytes: &[u8], preset: &str) -> Result<Vec<u8>, JsValue> {
    docx::fix_docx(bytes, preset).map_err(|err| JsValue::from_str(&err))
}

/// Doc Format: perbaiki .docx sesuai profil kustom (JSON) dan pilihan
/// kelompok aturan (JSON daftar centang), kembalikan byte .docx.
#[wasm_bindgen]
pub fn fix_docx_profile_rules(
    bytes: &[u8],
    profile_json: &str,
    rules_json: &str,
) -> Result<Vec<u8>, JsValue> {
    docx::fix_docx_profile_rules(bytes, profile_json, rules_json)
        .map_err(|err| JsValue::from_str(&err))
}
