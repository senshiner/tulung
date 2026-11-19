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
/// banding sebelum/sesudah (memastikan isi tidak berubah).
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

/// Doc Format: pindai .docx terhadap profil kustom (JSON), kembalikan
/// laporan sebagai JSON string.
#[wasm_bindgen]
pub fn scan_docx_profile(bytes: &[u8], profile_json: &str) -> Result<String, JsValue> {
    docx::scan_docx_profile(bytes, profile_json).map_err(|err| JsValue::from_str(&err))
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
