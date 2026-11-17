//! Mesin Tulung: logika semua tools, dikompilasi ke WebAssembly.
//!
//! Tiap tool diekspos sebagai fungsi `#[wasm_bindgen]` yang dipanggil
//! shell web (`web/`). Semua pemrosesan jalan di browser, dokumen
//! pengguna tidak pernah diunggah ke server.

use wasm_bindgen::prelude::*;

/// Versi tulung-core yang sedang berjalan di browser.
#[wasm_bindgen]
pub fn tulung_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
