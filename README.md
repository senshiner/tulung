<h1 align="center">Tulung</h1>

<p align="center"><strong>Tugasmu, dibantu sampai beres.</strong></p>
<p align="center">Pembantu tugas mahasiswa: gambar ke PDF, Doc Format (.docx), tulis tangan.<br>Tanpa daftar, tanpa install. Semua proses jalan di browser.</p>

<p align="center">
  <a href="https://github.com/senshiner/tulung/actions/workflows/ci.yml"><img src="https://github.com/senshiner/tulung/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <img src="https://img.shields.io/badge/license-MIT-green.svg" alt="Lisensi MIT">
  <img src="https://img.shields.io/badge/Rust-1.100-orange.svg" alt="Rust">
  <img src="https://img.shields.io/badge/WebAssembly-654FF0.svg" alt="WebAssembly">
</p>

<p align="center">
  <a href="https://senshiner.github.io/tulung/">Website</a> &middot;
  <a href="#mulai-cepat">Mulai Cepat</a> &middot;
  <a href="#cara-kerja">Cara Kerja</a> &middot;
  <a href="#roadmap">Roadmap</a> &middot;
  <a href="#berkontribusi">Berkontribusi</a>
</p>

## Apa itu Tulung

Tulung (bahasa Jawa: tolong, bantu) adalah satu aplikasi web berisi banyak tools pembantu tugas mahasiswa.

Bedanya dengan tools sejenis: semua pemrosesan jalan di browser lewat WebAssembly. Dokumen kamu tidak diunggah ke server mana pun.

| Tool | Status | Keterangan |
| --- | --- | --- |
| Gambar ke PDF | Siap dipakai | Foto tugas jadi satu PDF A4 |
| Doc Format | Siap dipakai | Format .docx otomatis sesuai pedoman dosen |
| Tulis Tangan | Segera | Teks jadi tulisan tangan (PDF) untuk latihan dan draf |

## Mulai Cepat

Butuh: Rust stable, target `wasm32-unknown-unknown`, dan `wasm-pack`.

```bash
git clone https://github.com/senshiner/tulung.git
cd tulung

rustup target add wasm32-unknown-unknown
cargo install wasm-pack

# Build kedua mesin WASM, lalu sajikan web/ secara statis
wasm-pack build --target web --out-dir ../../web/pkg-pdf crates/tulung-pdf
wasm-pack build --target web --out-dir ../../web/pkg-docx crates/tulung-docx
python3 -m http.server --directory web 8000
```

Buka http://localhost:8000 di browser.

## Cara Kerja

```
web/                  Shell web (HTML + CSS + JS, gaya neobrutalism)
crates/tulung-pdf/     Mesin gambar ke PDF, dikompilasi ke WebAssembly
crates/tulung-docx/    Mesin Doc Format (.docx), dikompilasi ke WebAssembly
  src/lib.rs          Fungsi tiap tool, dipanggil shell lewat wasm-bindgen
```

1. Pengguna memilih file di browser (File API).
2. Shell mengirim byte file ke fungsi WASM.
3. WASM memproses semuanya di dalam browser.
4. Hasil diunduh langsung, tanpa server.

## Roadmap

- **Tahap 0, Fondasi:** repo, workspace Cargo, shell web, CI. (selesai)
- **Tahap 1, Gambar ke PDF:** port my-pdf ke Rust + WASM, perbaiki bug endpoint dan gambar tinggi. (selesai)
- **Tahap 2, Doc Format:** pindai .docx, rapor pindai (informasi), perbaiki satu klik, regenerasi daftar isi/tabel/gambar/pustaka, isi identik kata per kata. (selesai)
- **Tahap 3, Tulis Tangan:** teks jadi PDF tulisan tangan.
- **Tahap 4, Profil dan berbagi:** penomoran (tabel/gambar) dan gaya sitasi di dalam profil. (Profil kustom, share lewat link, daftar pustaka, dan mode banding sudah jadi di Tahap 2.)

Perancangan lengkap ada di [docs/perancangan.md](docs/perancangan.md).

## Berkontribusi

Proyek ini masih tahap awal dan dibangun bergilir, satu tool selesai baru lanjut. Kalau mau ikut:

1. Fork repo ini, buat branch dari `main` (misal `doc-format`).
2. Commit dengan pesan Conventional Commits yang jelas (`feat:`, `fix:`, `docs:`, `chore:`).
3. Pastikan `cargo test --workspace` dan `wasm-pack build` lolos sebelum pull request.

## Lisensi

MIT. Lihat [LICENSE](LICENSE).
