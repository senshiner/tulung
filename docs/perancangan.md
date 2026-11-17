# Perancangan Tulung

Dokumen perancangan awal. Dibuat 18 November 2026, 00:02 WIB.
Nama sudah diputuskan: **Tulung**.

## 1. Nama dan visi

- **Nama:** Tulung, dari bahasa Jawa "tulung" (tolong, bantu).
- **Tagline (usulan):** "Tugasmu, dibantu sampai beres."
- **Visi:** satu aplikasi web pembantu tugas mahasiswa. Tanpa daftar, tanpa install. Semua proses jalan di browser, dokumen tidak diunggah ke server.
- **Sifat proyek:** pribadi, dibuat untuk membantu diri sendiri dulu, lalu seangkatan kalau sudah layak. Inspirasi: suite Craft (wordcraft, gridcraft, deckcraft, photocraft, figcraft) dan OpenCut, tools yang dibuat orang untuk kebutuhan mereka sendiri.
- **Prinsip bangun:** bergilir, bukan sekaligus. Satu tool selesai dan bisa dipakai, baru lanjut ke tool berikut.

## 2. Konsep produk

- Satu situs, banyak tools. Polanya seperti iLovePDF: tiap tool modul mandiri, berbagi shell, tema, dan penyimpanan profil.
- Tool yang direncanakan:
  1. Gambar ke PDF (mahasiswa paling sering pakai).
  2. Perapi Dokumen: format .docx otomatis sesuai pedoman dosen/kampus.
  3. Tulis Tangan: teks jadi tulisan tangan (PDF).
  4. Lainnya menyusul, ditambah bergilir.
- Target pengguna pertama: pembuatnya sendiri. Target berikut: mahasiswa Indonesia, mulai dari seangkatan.

## 3. Tech stack

Dipilih 17 November 2026: **Rust + WebAssembly**.

- **Inti:** Rust, dikompilasi ke WASM dengan wasm-pack dan wasm-bindgen. Semua logika berat (olah gambar, baca/tulis .docx, render tulisan tangan) jalan di browser.
- **Shell web:** TypeScript + HTML biasa, memanggil core WASM. Dipilih karena sudah dikuasai, jadi belajar Rust fokus di logika, bukan di framework. Opsi nanti: pindah ke Leptos atau Dioxus kalau mau full Rust.
- **Crate kunci (kandidat):**
  - `rdocx`: baca/tulis .docx, menjaga XML yang tidak dikenal tetap utuh (penting untuk janji "isi identik kata per kata"). Facade WASM-nya ada di workspace repo, tapi belum dipublikasi, jadi harus di-build dari source. Alternatif: `docx-rs` (paket `docx-wasm` sudah terbit, tapi lebih berorientasi membangun dokumen, bukan mengonsumsi).
  - `printpdf` / `lopdf`: membuat PDF.
  - `image`: olah gambar (crop, rotate, resize, filter).
  - `fontdue` / `ab_glyph` + `tiny-skia`: render tulisan tangan (font + garis goyang + kertas bergaris, resep dari render Vogel).
- **Repo (usulan, belum dibuat):** `senshiner/tulung`.
- **CI dan hosting:** GitHub Actions untuk build WASM dan deploy. Hosting demo: GitHub Pages (gratis, statis, cocok karena semua proses di browser) atau Vercel.

## 4. Arsitektur

- `tulung-core` (Rust lib, jadi WASM): tipe data bersama, mesin tiap tool, dan definisi profil pedoman (JSON).
- `tulung-web` (shell): landing, halaman tiap tool, file picker (File API), unduh hasil.
- `tulung-profiles`: preset bawaan dan profil kustom.
  - Preset standar umum: Times New Roman 12, spasi 1,5, margin 4-3-3-3.
  - Profil UNPAM: aturan pedoman kampus sendiri.
  - Profil kustom: pengguna bikin aturan sendiri (font, spasi, margin, gaya heading, penomoran, gaya sitasi), disimpan sebagai JSON, dibagikan lewat link (profil di-encode di URL, bukan dokumennya).
- **Privasi:** dokumen tidak pernah keluar dari browser. Yang dibagikan lewat link hanya aturannya, bukan isi tugas.

## 5. Roadmap

### Tahap 0, Fondasi
Repo, workspace Cargo, pipeline build WASM, shell web, CI, deploy demo kosong. Target: 1-2 sesi.

### Tahap 1, Gambar ke PDF
Port dari my-pdf (Next.js) ke Rust + WASM. Yang diperbaiki dari my-pdf:
- Bug endpoint: `pages/index.js` memanggil `/api/convert-multi`, padahal file API-nya `convert.js` (route benar `/api/convert`). Di versi baru tidak ada lagi backend, jadi bug ini hilang dengan sendirinya.
- Gambar tinggi meluber dari halaman A4: tambah batas tinggi saat resize.
- Tambahan: drag-reorder halaman, preset ekspor (kualitas cetak vs file kecil).
- Hasil: tool pertama yang bisa dipakai.

### Tahap 2, Perapi Dokumen (.docx)
Alur: upload .docx, pindai (font, ukuran, spasi, margin, heading), rapor checklist masalah, centang yang mau diperbaiki, perbaiki satu klik, unduh hasil.
- Janji inti: isi identik kata per kata, hanya format yang berubah.
- Preset: standar umum dan UNPAM.
- Daftar isi, daftar tabel, daftar gambar dibuat ulang (Tahap 2 akhir atau Tahap 4).
- Mode banding sebelum/sesudah (Tahap 4).

### Tahap 3, Tulis Tangan
Teks jadi PDF tulisan tangan: pilihan font tangan, garis agak goyang, kertas bergaris (resep sudah ada dari render jawaban Vogel).
- Posisi produk: untuk latihan dan draf. Bukan untuk dikumpulkan sebagai tulisan tangan asli, karena itu wilayah abu-abu.

### Tahap 4, Profil dan berbagi
Profil pedoman kustom, share lewat link, mode banding sebelum/sesudah, daftar pustaka dan gaya sitasi.

## 6. Aturan main

- Satu tahap sama dengan satu rilis yang bisa dipakai. Jangan bangun dua tool sekaligus.
- Jangan sentuh isi dokumen, hanya format (janji inti Tahap 2).
- UI tenang, sedikit warna, bahasa manusia. Anti-slop berlaku.
- Tiap tahap selesai, uji dengan dokumen nyata sendiri (makalah, tugas kuliah) sebelum lanjut.

## 7. Risiko dan yang belum diputuskan

- `rdocx`: facade WASM belum dipublikasi, harus build dari source. Verifikasi saat Tahap 0. Kalau mentok, evaluasi `docx-rs`.
- Round-trip .docx: risiko XML hilang saat tulis ulang. Mitigasi: hanya ubah properti format, jangan tulis ulang dokumen.
- Repo `senshiner/tulung`: publik atau privat, belum diputuskan. Usulan: publik, supaya bisa dipamerkan di portofolio.
- Nama modul formatter di dalam suite: "Perapi Dokumen" (usulan). Nama "DocForma" tidak dipakai karena sudah jadi produk orang (docforma.com).

## 8. Langkah berikut

1. Buatkan repo `senshiner/tulung`? Publik atau privat.
2. Mulai Tahap 0 (fondasi) sekarang?
