//! Doc Format: pindai dan perbaiki format .docx.
//!
//! Pindai membaca font, ukuran, spasi, margin, dan heading, lalu
//! melaporkan apa saja yang tidak sesuai preset. Perbaiki menerapkan
//! preset ke semua paragraf isi dan sel tabel, semua section, serta
//! font bawaan dokumen (docDefaults) supaya run tanpa format eksplisit
//! ikut berubah. Teks isi dokumen tidak berubah: hanya format yang
//! berubah. Daftar isi, tabel, dan gambar yang sudah ada dibangun
//! ulang (lihat bawah); daftar isi dan daftar tabel/gambar
//! dibuatkan bila belum ada (lihat bawah).
//!
//! Aturan ukuran: run dengan ukuran 14 pt atau lebih dianggap judul
//! (judul sampul, subjudul) — font-nya diseragamkan, ukurannya
//! dipertahankan. Tanpa aturan ini, judul sampul makalah ikut mengecil
//! jadi 12 pt. Heading diperlakukan sama: hanya font yang diubah.
//!
//! Perbaikan juga membuat ulang daftar: daftar isi (dari heading),
//! daftar tabel/gambar yang sudah ada, dan daftar pustaka yang sudah
//! ada. Dokumen yang punya heading tapi belum ada daftar isi akan
//! dibuatkan daftar isinya di awal isi. Daftar tabel/gambar yang
//! belum ada dibuatkan bila dokumen punya keterangan berfield SEQ
//! berlabel Tabel/Gambar (atau Table/Figure): daftarnya disisipkan
//! sesudah daftar isi, atau di awal isi bila tidak ada daftar isi.
//! Keterangan yang diketik manual tanpa field SEQ tidak dikumpulkan
//! daftar — sama seperti di Word, daftar tabel/gambar hanya membaca
//! keterangan berfield SEQ. Daftar pustaka TIDAK dibuatkan bila
//! belum ada — hanya yang sudah ada yang dibuat ulang.
//!
//! Batasan yang diketahui: header hanya dilaporkan (ada/tidak),
//! tidak ikut diperbaiki, karena rdocx 0.17 tidak membuka API tulis
//! untuk paragraf header. Footer ditulis oleh penomoran halaman
//! (lihat `apply_page_numbering`). Dokumen yang sudah punya footer
//! tetap bisa dinomori: footer asli pengguna menempel di section
//! isi (sectPr asal menjadi sectPr section penutup) dan disalin
//! ke section sampul dan bagian awal, jadi isinya tetap tampil di
//! semua halaman; paragraf PAGE disisip sesudah isi footer yang
//! ada, bukan menimpanya. Sampul menampilkan footer pengguna apa
//! adanya, tanpa tambahan nomor.
//! Nomor halaman di daftar isi dan daftar tabel/gambar dihitung
//! layout internal rdocx dan bisa meleset dari pagination
//! Word / LibreOffice (terukur sampai
//! 7 halaman pada dokumen 33 halaman): judul dan struktur daftar
//! benar, angka halaman adalah perkiraan. Pengguna perlu Perbarui
//! Field di Word/LibreOffice untuk angka eksak.
//!
//! Penomoran halaman berjenjang (sampul tanpa nomor, bagian awal
//! romawi kecil, isi arab mulai 1) diterapkan bila kelompok
//! `page_numbering` dicentang dan batasnya dipilih pengguna
//! (akhir sampul, awal isi — deteksi otomatis tidak andal, jadi
//! batas tidak ditebak). Skema: 3 section, 2 break. Section 1
//! (sampul) tanpa nomor (footer kosong, atau footer bawaan
//! pengguna bila dokumen punya footer); section 2 (bagian awal)
//! footer PAGE dengan `pgNumType` romawi kecil mulai 1; section 3
//! (isi) footer PAGE dengan `pgNumType` arab mulai 1. Format nomor
//! (`w:fmt`) tidak dimodelkan rdocx 0.17, jadi ditulis lewat
//! `replace_section_properties_xml` (API resmi rdocx). Break dibuat
//! `Paragraph::set_section_break`; sectPr baru itu default Letter,
//! jadi geometri A4 + margin dokumen disalin ke tiap section baru.
//!
//! Selain profil, perbaikan bisa dibatasi per kelompok aturan
//! (ApplyRules): pengguna memilih di UI kelompok mana yang diterapkan.
//! Tiga kelompok tidak datang dari profil melainkan aturan baku
//! makalah: indentasi baris pertama 1,25 cm, perataan justify, dan
//! jarak antarparagraf 0 pt, semuanya hanya untuk paragraf isi
//! (bukan heading, bukan entri daftar, bukan judul, bukan paragraf
//! yang memang diatur tengah/kanan seperti keterangan tabel).
//!
//! Dua jenis paragraf tidak ikut dirapikan sama sekali, di semua
//! kelompok aturan, dan tidak dinilai di pindai (hanya dilaporkan
//! sebagai info): keterangan gambar/tabel dan sampul. Keterangan
//! dikenali dari style Caption (id atau nama style "caption") atau
//! teks berlabel nomor di awal paragraf ("Gambar 3.1 ...",
//! "Tabel 4.2 ...", "Figure 1 ...", "Table 2 ...") — formatnya
//! memang beda dari isi (lebih kecil, sering miring) dan harus
//! dipertahankan. Pola teks sengaja ketat (label di awal, langsung
//! diikuti angka) supaya kalimat isi yang menyebut gambar/tabel
//! tidak ikut terlindungi. Entri daftar gambar/tabel bukan target:
//! itu entri daftar (style daftar), sudah ditangani jalur daftar.
//!
//! Sampul dideteksi sebagai paragraf dari awal isi sampai paragraf
//! pertama yang mengakhiri halaman pertama: paragraf yang memuat
//! page break, paragraf yang menutup sebuah section, atau paragraf
//! tepat sebelum paragraf ber-pageBreakBefore. Tanpa penanda itu
//! sampul TIDAK ditebak — dokumen tanpa break berperilaku persis
//! seperti biasa. Ini pendekatan: break pertama di tengah dokumen
//! pun dianggap akhir sampul, karena pagination asli tidak
//! tersimpan di .docx (lastRenderedPageBreak hanya petunjuk hasil
//! render terakhir dan sengaja tidak dipakai).

use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};

use rdocx::{
    Alignment, BreakKind, ContentFragment, ContentLocation, Document, HeaderFooterKind,
    HdrFtrType, Length, RunItemRef, SectionBreak,
};
use rdocx_oxml::CT_P;
use serde::{Deserialize, Serialize};

/// Run dengan ukuran sebesar ini atau lebih dianggap judul: font-nya
/// diseragamkan, ukurannya dipertahankan (lihat catatan modul).
const TITLE_MIN_SIZE_PT: f64 = 14.0;

/// Indentasi baris pertama paragraf isi menurut aturan baku (cm).
const FIRST_LINE_INDENT_CM: f64 = 1.25;

/// Profil pedoman format. Setiap aturan opsional: None berarti aspek
/// itu tidak diminta diubah (kolom dikosongkan di formulir profil).
/// Formulir mengisi semua kolom dengan angka default standar umum di
/// awal; pengguna boleh memakai default atau mengosongkannya.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub font: Option<String>,
    pub size_pt: Option<f64>,
    pub line_spacing: Option<f64>,
    pub margin_top_cm: Option<f32>,
    pub margin_right_cm: Option<f32>,
    pub margin_bottom_cm: Option<f32>,
    pub margin_left_cm: Option<f32>,
    pub heading_font: Option<String>,
}

impl Profile {
    /// Margin (atas, kanan, bawah, kiri) dalam cm, per sisi opsional.
    fn margins(&self) -> (Option<f32>, Option<f32>, Option<f32>, Option<f32>) {
        (
            self.margin_top_cm,
            self.margin_right_cm,
            self.margin_bottom_cm,
            self.margin_left_cm,
        )
    }
}

/// Profil "Standar Umum": Times New Roman 12, spasi 1,5,
/// margin atas 4, kanan 3, bawah 3, kiri 3 cm.
pub fn builtin_profile() -> Profile {
    Profile {
        name: "Standar Umum".to_string(),
        font: Some("Times New Roman".to_string()),
        size_pt: Some(12.0),
        line_spacing: Some(1.5),
        margin_top_cm: Some(4.0),
        margin_right_cm: Some(3.0),
        margin_bottom_cm: Some(3.0),
        margin_left_cm: Some(3.0),
        heading_font: Some("Times New Roman".to_string()),
    }
}

/// Profil "Makalah": seperti Standar Umum, tetapi margin kiri 4 cm
/// (ruang jilid) — atas 4, kanan 3, bawah 3, kiri 4 cm.
pub fn makalah_profile() -> Profile {
    Profile {
        name: "Makalah".to_string(),
        margin_left_cm: Some(4.0),
        ..builtin_profile()
    }
}

/// Baca preset bawaan dari nama yang dikirim shell web.
pub fn preset(name: &str) -> Result<Profile, String> {
    match name {
        "umum" => Ok(builtin_profile()),
        "makalah" => Ok(makalah_profile()),
        other => Err(format!("Preset tidak dikenal: {other}")),
    }
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

/// Batas penomoran halaman: indeks paragraf isi (urutan
/// `Document::paragraphs`, mulai 0) terakhir sampul dan pertama isi.
/// Dipilih pengguna di UI; tidak ditebak otomatis karena batas
/// sampul/pengantar/isi tidak bisa dideteksi andal dari dokumen
/// sembarang.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PageNumbering {
    pub cover_end: usize,
    pub content_start: usize,
}

/// Pilihan kelompok aturan yang diterapkan perbaikan. Dikirim shell
/// web dari daftar centang "Aturan yang diterapkan": kelompok yang
/// tidak dicentang dilewati perbaikan. Kolom yang tidak dikirim
/// berarti true (semua diterapkan), KECUALI `page_numbering` yang
/// default-nya false: penomoran butuh batas yang dipilih pengguna,
/// jadi tanpa pilihan eksplisit ia mati. `page_bounds` hanya dibaca
/// bila `page_numbering` dicentang; dicentang tanpa batas membuat
/// perbaikan ditolak dengan pesan yang jelas (bukan diam-diam dilewati).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ApplyRules {
    #[serde(default = "default_true")]
    pub margins: bool,
    #[serde(default = "default_true")]
    pub font: bool,
    #[serde(default = "default_true")]
    pub size: bool,
    #[serde(default = "default_true")]
    pub spacing: bool,
    #[serde(default = "default_true")]
    pub headings: bool,
    #[serde(default = "default_true")]
    pub indent: bool,
    #[serde(default = "default_true")]
    pub alignment: bool,
    #[serde(default = "default_true")]
    pub paragraph_spacing: bool,
    /// Mati bila tidak dikirim: butuh batas dari pengguna.
    #[serde(default = "default_false")]
    pub page_numbering: bool,
    /// Batas penomoran halaman (None bila pengguna belum memilih).
    #[serde(default)]
    pub page_bounds: Option<PageNumbering>,
    #[serde(default = "default_true")]
    pub lists: bool,
}

impl Default for ApplyRules {
    fn default() -> Self {
        Self {
            margins: true,
            font: true,
            size: true,
            spacing: true,
            headings: true,
            indent: true,
            alignment: true,
            paragraph_spacing: true,
            page_numbering: false,
            page_bounds: None,
            lists: true,
        }
    }
}

impl ApplyRules {
    /// Aturan jalur lama (fix_docx / fix_docx_profile tanpa pilihan):
    /// hanya kelompok yang diatur profil yang diterapkan, persis
    /// perilaku sebelum ada pilihan aturan. Kelompok aturan baku
    /// (indentasi, perataan, jarak antarparagraf) mati di jalur ini.
    fn legacy() -> Self {
        Self {
            indent: false,
            alignment: false,
            paragraph_spacing: false,
            page_numbering: false,
            ..Self::default()
        }
    }
}

/// Baca pilihan aturan dari JSON yang dikirim shell web. Kolom yang
/// tidak ada berarti true (semua diterapkan).
pub fn rules_from_json(json: &str) -> Result<ApplyRules, String> {
    serde_json::from_str(json).map_err(|err| format!("Pilihan aturan tidak valid: {err}"))
}

/// Baca profil kustom dari JSON yang dikirim shell web (formulir
/// profil). Kolom kosong di JSON berarti None (aspek itu tidak diubah).
pub fn profile_from_json(json: &str) -> Result<Profile, String> {
    serde_json::from_str(json).map_err(|err| format!("Profil tidak valid: {err}"))
}

/// Satu baris hitungan untuk laporan: nilai tampil + jumlah kemunculan.
#[derive(Serialize)]
pub struct Counted {
    pub value: String,
    pub count: usize,
}

/// Satu temuan dalam laporan pindai. `group` adalah kelompok aturan
/// di daftar centang UI (margins, font, size, spacing, headings,
/// indent, alignment, paragraph_spacing, lists, info) supaya shell
/// bisa menampilkan status tiap kelompok; `kind` adalah jenis temuan
/// (font, size, spacing, margin, heading, indent, alignment,
/// paragraph_spacing, info). `paragraphs` adalah indeks paragraf
/// (1-based, urutan yang sama dengan `ScanReport::paragraphs`)
/// tempat temuan terjadi — kosong untuk temuan yang bukan milik
/// paragraf tertentu (margin, font/ukuran bawaan dokumen).
/// `expected` adalah nilai target dari profil/aturan baku; kosong
/// bila tidak ada target (temuan info murni). `message` sudah
/// dirangkai dari keduanya dalam format faktual: paragraf tempat
/// temuan terjadi, nilai yang ditemukan, nilai yang seharusnya.
#[derive(Debug, Serialize)]
pub struct Issue {
    pub kind: &'static str,
    pub group: &'static str,
    pub message: String,
    pub count: usize,
    pub paragraphs: Vec<usize>,
    pub expected: String,
}

/// Laporan hasil pindai, dikirim ke shell web sebagai JSON.
/// Ringkasan font/ukuran mengikuti rincian temuan: `fonts`/`sizes`
/// hanya menghitung run paragraf isi, `heading_fonts`/`heading_sizes`
/// hanya run heading. Dulu ringkasan menghitung semua run termasuk
/// heading, sementara bullet rincian memisahnya — angkanya terkesan
/// kontradiktif (mis. ringkasan "Arial 3" tapi bullet "Arial 1").
#[derive(Serialize)]
pub struct ScanReport {
    pub paragraphs: usize,
    pub tables: usize,
    pub sections: usize,
    pub has_header: bool,
    pub has_footer: bool,
    pub fonts: Vec<Counted>,
    pub heading_fonts: Vec<Counted>,
    pub sizes: Vec<Counted>,
    pub heading_sizes: Vec<Counted>,
    pub spacings: Vec<Counted>,
    pub margins_cm: Option<[f32; 4]>,
    pub default_font: Option<String>,
    pub default_size_pt: Option<f64>,
    pub issues: Vec<Issue>,
    pub lists: ListInfo,
}

/// Status daftar dalam dokumen (hanya informasi, bukan masalah).
#[derive(Serialize)]
pub struct ListInfo {
    /// Jumlah entri daftar isi; None bila tidak ada field daftar isi.
    pub toc_entries: Option<usize>,
    /// (label, jumlah entri) tiap daftar tabel/gambar yang sudah ada.
    /// Entri None bila jumlah tidak bisa dihitung (field sederhana).
    pub tables: Vec<ListTableInfo>,
    /// Label field SEQ keterangan yang ada di dokumen (mis. "Tabel",
    /// "Gambar"), urut kemunculan pertama, tanpa duplikat. Dasar
    /// pembuatan daftar tabel/gambar yang belum ada.
    pub caption_labels: Vec<String>,
    /// Jumlah entri daftar pustaka; None bila tidak ada fieldnya.
    pub bibliography_entries: Option<usize>,
    /// Jumlah heading (menentukan apakah daftar isi akan dibuatkan).
    pub headings: usize,
}

/// Satu daftar tabel/gambar dalam laporan pindai.
#[derive(Serialize)]
pub struct ListTableInfo {
    pub label: String,
    pub entries: Option<usize>,
}

/// Format angka: buang ,0 di belakang, pakai koma desimal.
fn fmt_num(value: f64) -> String {
    if (value - value.round()).abs() < 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}").replace('.', ",")
    }
}

/// Format angka selalu satu desimal, koma desimal (untuk margin,
/// mengikuti tampilan ringkasan pindai di shell: "3,0 cm").
fn fmt_1dp(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

/// Awalan lokasi temuan: "Paragraf 12, 18, 25, 31, 40 (+7 lain)".
/// Maksimal lima indeks pertama disebut; sisanya diringkas.
fn para_prefix(paragraphs: &[usize]) -> String {
    let shown: Vec<String> = paragraphs.iter().take(5).map(usize::to_string).collect();
    let mut text = format!("Paragraf {}", shown.join(", "));
    if paragraphs.len() > 5 {
        text += &format!(" (+{} lain)", paragraphs.len() - 5);
    }
    text
}

/// Catat satu kemunculan temuan pada paragraf `idx` ke ember
/// `key`: hitungan bertambah, indeks paragraf ditambahkan sekali
/// per paragraf (urutan paragraf menaik, jadi cukup cek yang
/// terakhir).
fn bump(
    map: &mut BTreeMap<String, (usize, Vec<usize>)>,
    key: String,
    idx: usize,
) {
    let entry = map.entry(key).or_insert((0, Vec::new()));
    entry.0 += 1;
    if entry.1.last() != Some(&idx) {
        entry.1.push(idx);
    }
}

fn is_heading(style_id: Option<&str>) -> bool {
    style_id
        .map(|s| s.to_lowercase().contains("heading"))
        .unwrap_or(false)
}

/// Kunci style ternormalisasi untuk perbandingan: huruf kecil,
/// tanpa spasi (id style Word membuang spasi dari namanya).
fn style_key(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Id style yang merupakan style keterangan (Caption bawaan Word):
/// id-nya sendiri "caption" atau nama style-nya "caption". Nama
/// style bawaan disimpan styles.xml sebagai "caption" apa pun
/// bahasa UI Word, jadi nama bisa dipercaya.
fn caption_style_ids(doc: &Document) -> HashSet<String> {
    let mut ids = HashSet::new();
    for style in doc.styles() {
        let name_match = style
            .name()
            .map(|name| style_key(name) == "caption")
            .unwrap_or(false);
        if style_key(style.style_id()) == "caption" || name_match {
            ids.insert(style.style_id().to_string());
        }
    }
    ids
}

/// Paragraf keterangan gambar/tabel: style Caption, atau teks
/// diawali label berkode nomor. Lihat catatan modul soal ketatnya
/// pola teks ini.
fn is_caption(style_id: Option<&str>, text: &str, caption_ids: &HashSet<String>) -> bool {
    if let Some(id) = style_id {
        if style_key(id) == "caption" || caption_ids.contains(id) {
            return true;
        }
    }
    let trimmed = text.trim_start();
    ["Gambar", "Tabel", "Figure", "Table"]
        .iter()
        .any(|label| {
            trimmed
                .strip_prefix(label)
                .map(|rest| {
                    rest.trim_start()
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        })
}

/// Paragraf memuat run page break. lastRenderedPageBreak bukan
/// page break (hanya petunjuk pagination render terakhir Word)
/// dan item-nya terpisah di rdocx, jadi tidak ikut terhitung.
fn has_page_break(p: &rdocx::ParagraphRef) -> bool {
    p.runs().any(|run| {
        run.items()
            .any(|item| matches!(item, RunItemRef::Break(BreakKind::Page)))
    })
}

/// Nama lokal sebuah tag XML (bagian sesudah awalan namespace).
fn local_name(tag: &str) -> &str {
    tag.rsplit(':').next().unwrap_or(tag)
}

/// Bendera akhir section per paragraf isi (urut
/// `Document::paragraphs`): true bila pPr paragraf mengandung
/// `<w:sectPr>`, artinya paragraf itu menutup sebuah section.
/// rdocx 0.17 tidak membuka pembacaan sectPr paragraf, jadi
/// word/document.xml dipindai langsung dengan pemindai tag ringan.
/// Paragraf teratas adalah `<w:p>` yang tidak berada di dalam
/// `<w:tbl>` maupun `<w:p>` lain; paragraf di dalam sdt (content
/// control) tingkat isi ikut dihitung, sama seperti
/// `Document::paragraphs` yang masuk ke content control. Hanya
/// sectPr di pPr PERTAMA yang merupakan anak langsung paragraf
/// yang dihitung (sectPr tingkat body di luar paragraf diabaikan).
fn section_end_flags(xml: &str) -> Vec<bool> {
    let Some(body_start) = xml.find("<w:body") else {
        return Vec::new();
    };
    let Some(body_tag_end) = xml[body_start..].find('>') else {
        return Vec::new();
    };
    let mut rest = &xml[body_start + body_tag_end + 1..];
    let mut flags: Vec<bool> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    // Paragraf teratas yang sedang terbuka: (kedalaman tumpukan
    // sesudah <w:p> masuk, pPr pertamanya sudah ketemu, ada sectPr).
    let mut open_para: Option<(usize, bool, bool)> = None;
    let mut in_ppr = false;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else { break };
        let raw_tag = &after[1..gt];
        rest = &after[gt + 1..];
        if raw_tag.starts_with('?') || raw_tag.starts_with('!') {
            continue;
        }
        if let Some(name) = raw_tag.strip_prefix('/') {
            let local = local_name(name.trim());
            if local == "pPr" {
                in_ppr = false;
            }
            if local == "p" {
                if let Some((depth, _, has_sect)) = open_para {
                    if stack.len() == depth {
                        flags.push(has_sect);
                        open_para = None;
                    }
                }
            }
            if let Some(pos) = stack.iter().rposition(|n| n == local) {
                stack.truncate(pos);
            }
            continue;
        }
        let self_closing = raw_tag.ends_with('/');
        let name_part = raw_tag.trim_end_matches('/').trim();
        let local = local_name(name_part.split_whitespace().next().unwrap_or(""));
        match local {
            "p" if open_para.is_none() && !stack.iter().any(|n| n == "tbl") => {
                if self_closing {
                    flags.push(false);
                } else {
                    open_para = Some((stack.len() + 1, false, false));
                    in_ppr = false;
                }
            }
            "pPr" if !self_closing => {
                if let Some((depth, seen, has_sect)) = open_para {
                    if !seen && stack.len() == depth {
                        in_ppr = true;
                        open_para = Some((depth, true, has_sect));
                    }
                }
            }
            "sectPr" => {
                if in_ppr {
                    if let Some((depth, seen, _)) = open_para {
                        open_para = Some((depth, seen, true));
                    }
                }
            }
            _ => {}
        }
        if !self_closing {
            stack.push(local.to_string());
        }
    }
    flags
}

/// Indeks paragraf terakhir sampul (urut `Document::paragraphs`),
/// atau None bila sampul tidak terdeteksi. Lihat catatan modul
/// soal heuristik dan batasnya. Bendera sectPr dari pemindaian XML
/// hanya dipercaya bila jumlah paragrafnya sama persis dengan
/// jumlah paragraf menurut rdocx; selisih berarti kedua urutan
/// tidak sejajar dan penanda section diabaikan.
fn cover_end_index(doc: &Document, xml: &str) -> Option<usize> {
    let paras = doc.paragraphs();
    let sect_flags = section_end_flags(xml);
    let sect_trusted = sect_flags.len() == paras.len();
    for (i, p) in paras.iter().enumerate() {
        if has_page_break(p) {
            return Some(i);
        }
        if sect_trusted && sect_flags[i] {
            return Some(i);
        }
        if paras
            .get(i + 1)
            .map(|next| next.is_page_break_before())
            .unwrap_or(false)
        {
            return Some(i);
        }
    }
    None
}

/// Satu paragraf yang dipindai: gaya heading atau bukan, spasi,
/// daftar run (font, ukuran) di dalamnya, dan properti paragraf
/// untuk aturan baku (indentasi, perataan, jarak antarparagraf).
/// Paragraf keterangan dan sampul ditandai supaya dilewati semua
/// penilaian (formatnya dipertahankan, bukan dinilai).
struct ParaStat {
    heading: bool,
    spacing: Option<f64>,
    runs: Vec<(Option<String>, Option<f64>)>,
    in_table: bool,
    first_line_indent_cm: Option<f64>,
    alignment: Option<Alignment>,
    space_before_pt: Option<f64>,
    space_after_pt: Option<f64>,
    caption: bool,
    cover: bool,
}

/// Paragraf judul: semua run berukuran judul (>= 14 pt). Judul
/// sampul dan sejenisnya dikecualikan dari aturan baku paragraf isi.
fn is_title(runs: &[(Option<String>, Option<f64>)]) -> bool {
    !runs.is_empty()
        && runs
            .iter()
            .all(|(_, size)| size.map(|s| s >= TITLE_MIN_SIZE_PT).unwrap_or(false))
}

fn collect_paragraphs(
    doc: &Document,
    cover_end: Option<usize>,
    caption_ids: &HashSet<String>,
) -> Vec<ParaStat> {
    let mut out = Vec::new();
    // Paragraf entri daftar dilewati: isinya dibuat ulang mesin,
    // bukan ditulis pengguna, jadi tidak dinilai terhadap profil.
    // Paragraf kosong dilewati juga (termasuk paragraf pembuka dan
    // penutup field daftar, yang memang tidak berisi kata).
    for (i, p) in doc.paragraphs().iter().enumerate() {
        if is_list_style(p.style_id()) || p.text().trim().is_empty() {
            continue;
        }
        let cover = cover_end.map(|end| i <= end).unwrap_or(false);
        out.push(para_stat(p, false, caption_ids, cover));
    }
    for table in doc.tables() {
        for row in 0..table.row_count() {
            for col in 0..table.column_count() {
                if let Some(cell) = table.cell(row, col) {
                    for p in cell.paragraphs() {
                        if is_list_style(p.style_id()) || p.text().trim().is_empty() {
                            continue;
                        }
                        out.push(para_stat(&p, true, caption_ids, false));
                    }
                }
            }
        }
    }
    out
}

fn para_stat(
    p: &rdocx::ParagraphRef,
    in_table: bool,
    caption_ids: &HashSet<String>,
    cover: bool,
) -> ParaStat {
    ParaStat {
        heading: is_heading(p.style_id()),
        spacing: p.line_spacing_multiple(),
        runs: p
            .runs()
            .map(|r| (r.font_name().map(str::to_string), r.size()))
            .collect(),
        in_table,
        first_line_indent_cm: p.first_line_indent().map(|len| len.to_cm()),
        alignment: p.alignment(),
        space_before_pt: p.space_before().map(|len| len.to_pt()),
        space_after_pt: p.space_after().map(|len| len.to_pt()),
        caption: is_caption(p.style_id(), &p.text(), caption_ids),
        cover,
    }
}

/// Pindai .docx dan laporkan temuan terhadap preset, sebagai JSON.
pub fn scan_docx(bytes: &[u8], preset_name: &str) -> Result<String, String> {
    let profile = preset(preset_name)?;
    let doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    let report = scan(&doc, &profile, bytes);
    serde_json::to_string(&report).map_err(|err| err.to_string())
}

/// Pindai .docx terhadap profil kustom (JSON), sebagai JSON.
pub fn scan_docx_profile(bytes: &[u8], profile_json: &str) -> Result<String, String> {
    let profile = profile_from_json(profile_json)?;
    let doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    let report = scan(&doc, &profile, bytes);
    serde_json::to_string(&report).map_err(|err| err.to_string())
}

/// Daftar paragraf isi dokumen (urutan `Document::paragraphs`),
/// untuk pemilih batas penomoran halaman di shell web. Indeks di
/// sini adalah indeks yang dipakai `PageNumbering`: paragraf tabel
/// tidak termasuk, karena section break hanya berlaku di alur isi.
pub fn docx_paragraphs(bytes: &[u8]) -> Result<String, String> {
    let doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    let list: Vec<serde_json::Value> = doc
        .paragraphs()
        .iter()
        .enumerate()
        .map(|(index, p)| {
            serde_json::json!({"index": index, "text": p.text()})
        })
        .collect();
    serde_json::to_string(&list).map_err(|err| err.to_string())
}

/// Isi word/document.xml dari byte .docx.
fn document_xml(bytes: &[u8]) -> Result<String, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|err| err.to_string())?;
    let mut file = archive
        .by_name("word/document.xml")
        .map_err(|err| err.to_string())?;
    let mut xml = String::new();
    file.read_to_string(&mut xml).map_err(|err| err.to_string())?;
    Ok(xml)
}

/// Satu field daftar yang ditemukan di word/document.xml.
struct FieldSpan {
    kind: FieldKind,
    label: Option<String>,
    /// Jumlah entri di antara separate dan end; None bila tidak
    /// bisa dihitung (field sederhana tanpa pemisah paragraf).
    entries: Option<usize>,
}

enum FieldKind {
    Toc,
    Figures,
    Bibliography,
}

/// Uraikan entitas XML yang umum di dalam teks instruksi field.
fn decode_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Pecah instruksi field jadi token; teks berpetik tetap satu token
/// tanpa tanda petik (mis. `TOC \o "1-3"` -> ["TOC", "\\o", "1-3"]).
fn split_field_tokens(instr: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in instr.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Tentukan jenis field dari token instruksinya.
fn classify_field(tokens: &[String]) -> Option<(FieldKind, Option<String>)> {
    match tokens.first().map(String::as_str) {
        Some("TOC") => {
            let mut i = 1;
            while i < tokens.len() {
                if let Some(switch) = tokens[i].strip_prefix('\\') {
                    match switch {
                        "o" => return Some((FieldKind::Toc, None)),
                        "c" => {
                            return Some((FieldKind::Figures, tokens.get(i + 1).cloned()));
                        }
                        _ => {}
                    }
                }
                i += 1;
            }
            None
        }
        Some("BIBLIOGRAPHY") => Some((FieldKind::Bibliography, None)),
        _ => None,
    }
}

/// Hitung paragraf di antara pemisah (separate) dan penutup (end)
/// sebuah field kompleks. Entri daftar adalah satu paragraf tiap
/// entri; skeleton kosong menghasilkan 0.
fn complex_entries(xml: &str, instr_end: usize) -> Option<usize> {
    let after_instr = &xml[instr_end..];
    let separate = after_instr.find("fldCharType=\"separate\"")? + instr_end;
    let end = xml[separate..].find("fldCharType=\"end\"")? + separate;
    let region_start = xml[separate..].find("</w:p>")? + separate + "</w:p>".len();
    let region_end = rfind_paragraph_start(&xml[..end])?;
    if region_end <= region_start {
        return Some(0);
    }
    let region = &xml[region_start..region_end];
    let mut count = 0;
    let mut rest = region;
    while let Some(pos) = rest.find("<w:p") {
        let after = rest[pos + 4..].chars().next();
        if matches!(after, Some('>') | Some(' ')) {
            count += 1;
        }
        rest = &rest[pos + 4..];
    }
    Some(count)
}

/// Posisi awal paragraf terakhir sebelum `before`: cari "<w:p" yang
/// diikuti '>' atau spasi (bukan "<w:pPr" properti paragraf).
fn rfind_paragraph_start(xml: &str) -> Option<usize> {
    let mut end = xml.len();
    loop {
        let pos = xml[..end].rfind("<w:p")?;
        let after = xml[pos + 4..].chars().next();
        if matches!(after, Some('>') | Some(' ')) {
            return Some(pos);
        }
        end = pos;
    }
}

/// Hitung entri daftar pustaka. rdocx (dan Word) menulis entri
/// pertama sebaris di paragraf field (setelah separate), entri
/// berikut sebagai paragraf berstyle Bibliography. Hitung paragraf
/// saja memberi 0 untuk pustaka satu entri.
fn bibliography_entries(xml: &str, instr_end: usize) -> Option<usize> {
    let after_instr = &xml[instr_end..];
    let separate = after_instr.find("fldCharType=\"separate\"")? + instr_end;
    let end = xml[separate..].find("fldCharType=\"end\"")? + separate;
    let para_end = xml[separate..].find("</w:p>")? + separate;
    // Entri pertama: ada teks di antara separate dan akhir paragraf.
    let mut count = 0;
    let mut rest = &xml[separate..para_end];
    while let Some(pos) = rest.find("<w:t") {
        let Some(gt) = rest[pos..].find('>') else { break };
        let text_start = pos + gt + 1;
        let Some(close) = rest[text_start..].find("</w:t>") else {
            break;
        };
        if !rest[text_start..text_start + close].trim().is_empty() {
            count = 1;
            break;
        }
        rest = &rest[text_start + close..];
    }
    // Entri berikut: paragraf berstyle Bibliography di region.
    let region_start = para_end + "</w:p>".len();
    let region_end = rfind_paragraph_start(&xml[..end])?;
    if region_end > region_start {
        count += xml[region_start..region_end]
            .matches("<w:pStyle w:val=\"Bibliography\"")
            .count();
    }
    Some(count)
}
/// Field kompleks (begin/instr/separate/end) dan field sederhana
/// (fldSimple) sama-sama dikenali; jumlah entri field sederhana
/// dihitung dari PAGEREF di dalamnya (pustaka: tidak bisa dihitung).
fn field_spans(xml: &str) -> Vec<FieldSpan> {
    let mut spans = Vec::new();
    // Field kompleks: instruksi di elemen <w:instrText>.
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<w:instrText") {
        let tag_start = offset + pos;
        let Some(gt) = xml[tag_start..].find('>') else { break };
        let instr_start = tag_start + gt + 1;
        let Some(close) = xml[instr_start..].find("</w:instrText>") else {
            break;
        };
        let instr_end = instr_start + close;
        let instr = decode_entities(&xml[instr_start..instr_end]);
        if let Some((kind, label)) = classify_field(&split_field_tokens(&instr)) {
            let entries = match kind {
                FieldKind::Bibliography => bibliography_entries(xml, instr_end),
                _ => complex_entries(xml, instr_end),
            };
            spans.push(FieldSpan {
                kind,
                label,
                entries,
            });
        }
        offset = instr_end + "</w:instrText>".len();
    }
    // Field sederhana: instruksi di atribut w:instr <w:fldSimple>.
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<w:fldSimple") {
        let tag_start = offset + pos;
        let Some(gt) = xml[tag_start..].find('>') else { break };
        let tag = &xml[tag_start..tag_start + gt];
        let entries = tag
            .find("w:instr=\"")
            .and_then(|start| {
                let value_start = start + "w:instr=\"".len();
                let value_end = tag[value_start..].find('"')? + value_start;
                Some(decode_entities(&tag[value_start..value_end]))
            })
            .and_then(|instr| classify_field(&split_field_tokens(&instr)));
        let Some((kind, label)) = entries else {
            offset = tag_start + gt + 1;
            continue;
        };
        let body_start = tag_start + gt + 1;
        let body_end = xml[body_start..]
            .find("</w:fldSimple>")
            .map(|p| p + body_start)
            .unwrap_or(body_start);
        let entries = match kind {
            FieldKind::Bibliography => None,
            _ => Some(xml[body_start..body_end].matches("PAGEREF").count()),
        };
        spans.push(FieldSpan {
            kind,
            label,
            entries,
        });
        offset = body_end + "</w:fldSimple>".len();
    }
    spans
}

/// Status daftar dalam dokumen, untuk rapor pindai.
fn list_info(doc: &Document, bytes: &[u8]) -> ListInfo {
    let headings = doc
        .paragraphs()
        .iter()
        .filter(|p| is_heading(p.style_id()))
        .count();
    let mut info = ListInfo {
        toc_entries: None,
        tables: Vec::new(),
        caption_labels: Vec::new(),
        bibliography_entries: None,
        headings,
    };
    let Ok(xml) = document_xml(bytes) else {
        return info;
    };
    info.caption_labels = seq_caption_labels(&xml);
    for span in field_spans(&xml) {
        match span.kind {
            FieldKind::Toc => {
                if info.toc_entries.is_none() {
                    info.toc_entries = span.entries;
                }
            }
            FieldKind::Figures => {
                if let Some(label) = span.label {
                    info.tables.push(ListTableInfo {
                        label,
                        entries: span.entries,
                    });
                }
            }
            FieldKind::Bibliography => {
                if info.bibliography_entries.is_none() {
                    info.bibliography_entries = span.entries;
                }
            }
        }
    }
    info
}

/// Label field SEQ yang ada di dokumen (mis. "Tabel", "Gambar"),
/// urut kemunculan pertama, tanpa duplikat. Field SEQ adalah nomor
/// keterangan yang ditulis Word/rdocx saat menyisipkan keterangan;
/// daftar tabel/gambar (field TOC \c) hanya mengumpulkan keterangan
/// berfield SEQ — keterangan yang diketik manual tanpa field tidak
/// terbaca daftar, sama seperti di Word.
fn seq_caption_labels(xml: &str) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    let mut check = |instr: &str| {
        let tokens = split_field_tokens(&decode_entities(instr));
        if tokens.first().map(String::as_str) == Some("SEQ") {
            if let Some(label) = tokens.get(1) {
                if !labels.contains(label) {
                    labels.push(label.clone());
                }
            }
        }
    };
    // Field kompleks: instruksi di elemen <w:instrText>.
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<w:instrText") {
        let tag_start = offset + pos;
        let Some(gt) = xml[tag_start..].find('>') else { break };
        let instr_start = tag_start + gt + 1;
        let Some(close) = xml[instr_start..].find("</w:instrText>") else {
            break;
        };
        check(&xml[instr_start..instr_start + close]);
        offset = instr_start + close + "</w:instrText>".len();
    }
    // Field sederhana: instruksi di atribut w:instr <w:fldSimple>.
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<w:fldSimple") {
        let tag_start = offset + pos;
        let Some(gt) = xml[tag_start..].find('>') else { break };
        let tag = &xml[tag_start..tag_start + gt];
        if let Some(start) = tag.find("w:instr=\"") {
            let value_start = start + "w:instr=\"".len();
            if let Some(value_end) = tag[value_start..].find('"') {
                check(&tag[value_start..value_start + value_end]);
            }
        }
        offset = tag_start + gt + 1;
    }
    labels
}

/// Judul daftar dan pesan pindai untuk label SEQ keterangan:
/// (judul paragraf daftar, pesan "akan dibuatkan"). Hanya label
/// tabel/gambar yang dibuatkan daftarnya; label lain (mis.
/// "Persamaan") tidak.
fn figure_list_title(label: &str) -> Option<(&'static str, &'static str)> {
    match label.to_lowercase().as_str() {
        "tabel" | "table" => Some(("Daftar Tabel", "Tidak ada daftar tabel, akan dibuatkan")),
        "gambar" | "figure" => Some((
            "Daftar Gambar",
            "Tidak ada daftar gambar, akan dibuatkan",
        )),
        _ => None,
    }
}

/// Daftar tabel/gambar yang perlu dibuatkan: (label SEQ, judul)
/// untuk label keterangan tabel/gambar yang belum punya field
/// daftarnya. Urutan baku daftar tabel sebelum daftar gambar; satu
/// daftar per judul — label pertama yang cocok dipakai untuk
/// fieldnya. Kecocokan label dengan daftar yang sudah ada persis
/// (peka huruf besar/kecil), sama seperti pencocokan field TOC \c
/// dengan field SEQ saat daftar dibangun ulang.
fn missing_figure_lists(
    caption_labels: &[String],
    existing_labels: &[String],
) -> Vec<(String, &'static str)> {
    let mut missing = Vec::new();
    for wanted in ["Daftar Tabel", "Daftar Gambar"] {
        for label in caption_labels {
            let Some((title, _)) = figure_list_title(label) else {
                continue;
            };
            if title != wanted {
                continue;
            }
            if !existing_labels.iter().any(|e| e == label) {
                missing.push((label.clone(), title));
            }
            break;
        }
    }
    missing
}

fn scan(doc: &Document, profile: &Profile, bytes: &[u8]) -> ScanReport {
    let xml = document_xml(bytes).unwrap_or_default();
    let cover_end = cover_end_index(doc, &xml);
    let caption_ids = caption_style_ids(doc);
    let paras = collect_paragraphs(doc, cover_end, &caption_ids);
    let mut fonts: BTreeMap<String, usize> = BTreeMap::new();
    let mut heading_fonts: BTreeMap<String, usize> = BTreeMap::new();
    let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
    let mut heading_sizes: BTreeMap<String, usize> = BTreeMap::new();
    let mut spacings: BTreeMap<String, usize> = BTreeMap::new();
    let mut issues: Vec<Issue> = Vec::new();

    // Ringkasan mengikuti rincian: run heading dihitung terpisah,
    // supaya angka ringkasan sama dengan angka bullet temuan.
    // Paragraf sampul dan keterangan tidak dinilai sama sekali.
    for para in &paras {
        if para.cover || para.caption {
            continue;
        }
        for (font, size) in &para.runs {
            let name = font.clone().unwrap_or_else(|| "Bawaan".to_string());
            if para.heading {
                *heading_fonts.entry(name).or_insert(0) += 1;
            } else {
                *fonts.entry(name).or_insert(0) += 1;
            }
            let size_text = size
                .map(|s| format!("{} pt", fmt_num(s)))
                .unwrap_or_else(|| "Bawaan".to_string());
            if para.heading {
                *heading_sizes.entry(size_text).or_insert(0) += 1;
            } else {
                *sizes.entry(size_text).or_insert(0) += 1;
            }
        }
        if !para.heading {
            let spacing_text = para
                .spacing
                .map(fmt_num)
                .unwrap_or_else(|| "Bawaan".to_string());
            *spacings.entry(spacing_text).or_insert(0) += 1;
        }
    }

    // Temuan font (paragraf isi + heading dipisah pesannya).
    // Aspek yang tidak diatur profil (None) tidak dilaporkan.
    // Setiap ember mencatat hitungan kemunculan dan indeks
    // paragraf tempat temuan terjadi.
    let mut font_issue: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
    let mut heading_font_issue: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
    let mut size_issue: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
    let mut title_kept: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
    let mut spacing_issue: BTreeMap<String, (usize, Vec<usize>)> = BTreeMap::new();
    for (i, para) in paras.iter().enumerate() {
        let idx = i + 1;
        if para.cover || para.caption {
            continue;
        }
        for (font, size) in &para.runs {
            if let Some(name) = font {
                if let Some(target) = &profile.font {
                    if name != target && !para.heading {
                        bump(&mut font_issue, name.clone(), idx);
                    }
                }
                if let Some(target) = &profile.heading_font {
                    if name != target && para.heading {
                        bump(&mut heading_font_issue, name.clone(), idx);
                    }
                }
            }
            if !para.heading {
                if let Some(size) = size {
                    if *size >= TITLE_MIN_SIZE_PT {
                        // Judul: ukurannya dipertahankan, bukan masalah.
                        bump(
                            &mut title_kept,
                            format!("{} pt", fmt_num(*size)),
                            idx,
                        );
                    } else if let Some(target) = profile.size_pt {
                        if (*size - target).abs() > 0.01 {
                            bump(
                                &mut size_issue,
                                format!("{} pt", fmt_num(*size)),
                                idx,
                            );
                        }
                    }
                }
            }
        }
        if !para.heading {
            if let Some(target) = profile.line_spacing {
                match para.spacing {
                    Some(spacing) if (spacing - target).abs() < 0.01 => {}
                    Some(spacing) => {
                        bump(
                            &mut spacing_issue,
                            format!("spasi {}", fmt_num(spacing)),
                            idx,
                        );
                    }
                    None => {
                        bump(&mut spacing_issue, "spasi tidak diatur".to_string(), idx);
                    }
                }
            }
        }
    }
    if let Some(target) = &profile.font {
        for (name, (count, paragraphs)) in font_issue {
            issues.push(Issue {
                kind: "font",
                group: "font",
                message: format!(
                    "{}: font {name} — seharusnya {target}.",
                    para_prefix(&paragraphs)
                ),
                count,
                paragraphs,
                expected: target.clone(),
            });
        }
    }
    if let Some(target) = &profile.heading_font {
        for (name, (count, paragraphs)) in heading_font_issue {
            issues.push(Issue {
                kind: "heading",
                group: "headings",
                message: format!(
                    "{}: heading memakai font {name} — seharusnya {target}.",
                    para_prefix(&paragraphs)
                ),
                count,
                paragraphs,
                expected: target.clone(),
            });
        }
    }
    if let Some(target) = profile.size_pt {
        for (size_text, (count, paragraphs)) in size_issue {
            issues.push(Issue {
                kind: "size",
                group: "size",
                message: format!(
                    "{}: ukuran {size_text} — seharusnya {} pt.",
                    para_prefix(&paragraphs),
                    fmt_num(target)
                ),
                count,
                paragraphs,
                expected: format!("{} pt", fmt_num(target)),
            });
        }
    }
    for (size_text, (count, paragraphs)) in title_kept {
        issues.push(Issue {
            kind: "info",
            group: "size",
            message: format!(
                "{}: ukuran {size_text} dipertahankan sebagai judul.",
                para_prefix(&paragraphs)
            ),
            count,
            paragraphs,
            expected: String::new(),
        });
    }
    if let Some(target) = profile.line_spacing {
        for (finding, (count, paragraphs)) in spacing_issue {
            issues.push(Issue {
                kind: "spacing",
                group: "spacing",
                message: format!(
                    "{}: {finding} — seharusnya {}.",
                    para_prefix(&paragraphs),
                    fmt_num(target)
                ),
                count,
                paragraphs,
                expected: fmt_num(target),
            });
        }
    }

    // Aturan baku paragraf isi (bukan dari profil): indentasi baris
    // pertama, perataan justify, jarak antarparagraf 0 pt. Hanya
    // paragraf isi sejati yang dinilai: bukan heading, bukan di dalam
    // tabel, bukan judul. Paragraf yang memang diatur tengah/kanan
    // (keterangan tabel/gambar, judul bab) dikecualikan dari nilai
    // indentasi dan perataan.
    let mut indent_issue: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut alignment_issue: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut para_spacing_issue: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, para) in paras.iter().enumerate() {
        let idx = i + 1;
        if para.heading || para.in_table || para.cover || para.caption || is_title(&para.runs) {
            continue;
        }
        let deliberate = matches!(
            para.alignment,
            Some(Alignment::Center) | Some(Alignment::Right)
        );
        if !deliberate {
            match para.first_line_indent_cm {
                Some(cm) if (cm - FIRST_LINE_INDENT_CM).abs() < 0.05 => {}
                Some(cm) => indent_issue
                    .entry(format!("indentasi baris pertama {} cm", fmt_num(cm)))
                    .or_default()
                    .push(idx),
                None => indent_issue
                    .entry("tanpa indentasi baris pertama".to_string())
                    .or_default()
                    .push(idx),
            }
            match para.alignment {
                Some(Alignment::Justify) => {}
                Some(Alignment::Left) => alignment_issue
                    .entry("rata kiri".to_string())
                    .or_default()
                    .push(idx),
                _ => alignment_issue
                    .entry("perataan tidak diatur".to_string())
                    .or_default()
                    .push(idx),
            }
        }
        // Jarak antarparagraf: hanya nilai eksplisit yang terlihat;
        // yang tidak diatur mengikuti style dan tidak dinilai.
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = para.space_before_pt {
            if v.abs() > 0.01 {
                parts.push(format!("sebelum {} pt", fmt_num(v)));
            }
        }
        if let Some(v) = para.space_after_pt {
            if v.abs() > 0.01 {
                parts.push(format!("sesudah {} pt", fmt_num(v)));
            }
        }
        if !parts.is_empty() {
            para_spacing_issue
                .entry(format!("jarak antarparagraf {}", parts.join(" dan ")))
                .or_default()
                .push(idx);
        }
    }
    for (finding, paragraphs) in indent_issue {
        issues.push(Issue {
            kind: "indent",
            group: "indent",
            message: format!(
                "{}: {finding} — seharusnya 1,25 cm.",
                para_prefix(&paragraphs)
            ),
            count: paragraphs.len(),
            paragraphs,
            expected: "1,25 cm".to_string(),
        });
    }
    for (finding, paragraphs) in alignment_issue {
        issues.push(Issue {
            kind: "alignment",
            group: "alignment",
            message: format!(
                "{}: {finding} — seharusnya rata kanan-kiri.",
                para_prefix(&paragraphs)
            ),
            count: paragraphs.len(),
            paragraphs,
            expected: "rata kanan-kiri".to_string(),
        });
    }
    for (finding, paragraphs) in para_spacing_issue {
        issues.push(Issue {
            kind: "paragraph_spacing",
            group: "paragraph_spacing",
            message: format!(
                "{}: {finding} — seharusnya 0 pt.",
                para_prefix(&paragraphs)
            ),
            count: paragraphs.len(),
            paragraphs,
            expected: "0 pt".to_string(),
        });
    }

    // Font bawaan dokumen (docDefaults).
    let default_font = doc.default_font_name().map(str::to_string);
    let default_size_pt = doc.default_font_size().map(|len| len.to_pt());
    if let Some(name) = &default_font {
        if let Some(target) = &profile.font {
            if name != target {
                issues.push(Issue {
                    kind: "font",
                    group: "font",
                    message: format!("Font bawaan dokumen: {name} — seharusnya {target}."),
                    count: 1,
                    paragraphs: Vec::new(),
                    expected: target.clone(),
                });
            }
        }
    }
    if let Some(size) = default_size_pt {
        if let Some(target) = profile.size_pt {
            if (size - target).abs() > 0.01 {
                issues.push(Issue {
                    kind: "size",
                    group: "size",
                    message: format!(
                        "Ukuran bawaan dokumen: {} pt — seharusnya {} pt.",
                        fmt_num(size),
                        fmt_num(target)
                    ),
                    count: 1,
                    paragraphs: Vec::new(),
                    expected: format!("{} pt", fmt_num(target)),
                });
            }
        }
    }

    // Margin section pertama. Hanya sisi yang diatur profil yang dicek.
    let sections: Vec<_> = doc.sections().collect();
    let margins_cm = sections.first().and_then(|s| s.margins()).map(|m| {
        [
            m.0.to_cm() as f32,
            m.1.to_cm() as f32,
            m.2.to_cm() as f32,
            m.3.to_cm() as f32,
        ]
    });
    if let Some(margins) = margins_cm {
        let current = (margins[0], margins[1], margins[2], margins[3]);
        let target = profile.margins();
        let sides = [
            ("atas", current.0, target.0),
            ("kanan", current.1, target.1),
            ("bawah", current.2, target.2),
            ("kiri", current.3, target.3),
        ];
        let mut fragments: Vec<String> = Vec::new();
        let mut expected_sides: Vec<String> = Vec::new();
        for (name, now, want) in sides {
            if let Some(want) = want {
                if (now - want).abs() >= 0.1 {
                    fragments.push(format!(
                        "margin {name} {} cm — seharusnya {} cm",
                        fmt_1dp(now as f64),
                        fmt_1dp(want as f64)
                    ));
                    expected_sides.push(format!("{name} {} cm", fmt_1dp(want as f64)));
                }
            }
        }
        if !fragments.is_empty() {
            let mut message = fragments.join("; ");
            // Huruf pertama pesan kapital; fragmen sisi berikutnya
            // tetap kecil sesudah titik koma.
            message.replace_range(..1, &message[..1].to_uppercase());
            message.push('.');
            issues.push(Issue {
                kind: "margin",
                group: "margins",
                message,
                count: fragments.len(),
                paragraphs: Vec::new(),
                expected: expected_sides.join(", "),
            });
        }
    }

    let has_header = doc.header_text().map(|t| !t.trim().is_empty()).unwrap_or(false);
    let has_footer = doc.footer_text().map(|t| !t.trim().is_empty()).unwrap_or(false);
    if has_header || has_footer {
        let what = match (has_header, has_footer) {
            (true, true) => "Header dan footer",
            (true, false) => "Header",
            _ => "Footer",
        };
        issues.push(Issue {
            kind: "info",
            group: "info",
            message: format!("{what} tidak diubah."),
            count: 0,
            paragraphs: Vec::new(),
            expected: String::new(),
        });
    }

    // Sampul dan keterangan: ada, tapi memang tidak diubah —
    // dilaporkan sebagai info supaya pengguna tahu keduanya
    // disengaja dilewati, bukan terlewat.
    let cover_paragraphs: Vec<usize> = paras
        .iter()
        .enumerate()
        .filter(|(_, p)| p.cover)
        .map(|(i, _)| i + 1)
        .collect();
    if !cover_paragraphs.is_empty() {
        issues.push(Issue {
            kind: "info",
            group: "info",
            message: "Sampul tidak diubah.".to_string(),
            count: cover_paragraphs.len(),
            paragraphs: cover_paragraphs,
            expected: String::new(),
        });
    }
    let caption_paragraphs: Vec<usize> = paras
        .iter()
        .enumerate()
        .filter(|(_, p)| p.caption)
        .map(|(i, _)| i + 1)
        .collect();
    if !caption_paragraphs.is_empty() {
        issues.push(Issue {
            kind: "info",
            group: "info",
            message: "Keterangan gambar/tabel tidak diubah.".to_string(),
            count: caption_paragraphs.len(),
            paragraphs: caption_paragraphs,
            expected: String::new(),
        });
    }

    // Status daftar (hanya informasi, bukan masalah). Daftar pustaka
    // hanya dilaporkan bila sudah ada, karena perbaikan tidak
    // membuatkannya bila belum ada; daftar tabel/gambar juga
    // dilaporkan bila akan dibuatkan.
    let lists = list_info(doc, bytes);
    match lists.toc_entries {
        Some(n) => issues.push(Issue {
            kind: "info",
            group: "lists",
            message: format!("Daftar isi: {n} entri"),
            count: n,
            paragraphs: Vec::new(),
            expected: String::new(),
        }),
        None if lists.headings > 0 => issues.push(Issue {
            kind: "info",
            group: "lists",
            message: "Tidak ada daftar isi, akan dibuatkan".to_string(),
            count: 0,
            paragraphs: Vec::new(),
            expected: String::new(),
        }),
        None => issues.push(Issue {
            kind: "info",
            group: "lists",
            message: "Tidak ada daftar isi".to_string(),
            count: 0,
            paragraphs: Vec::new(),
            expected: String::new(),
        }),
    }
    for table in &lists.tables {
        match table.entries {
            Some(n) => issues.push(Issue {
                kind: "info",
                group: "lists",
                message: format!("Daftar {}: {n} entri", table.label),
                count: n,
                paragraphs: Vec::new(),
                expected: String::new(),
            }),
            None => issues.push(Issue {
                kind: "info",
                group: "lists",
                message: format!("Daftar {}: ada", table.label),
                count: 0,
                paragraphs: Vec::new(),
                expected: String::new(),
            }),
        }
    }
    // Keterangan berfield SEQ tanpa field daftarnya: daftarnya akan
    // dibuatkan perbaikan.
    let existing_labels: Vec<String> = lists.tables.iter().map(|t| t.label.clone()).collect();
    for (label, _) in missing_figure_lists(&lists.caption_labels, &existing_labels) {
        let (_, message) = figure_list_title(&label).unwrap();
        issues.push(Issue {
            kind: "info",
            group: "lists",
            message: message.to_string(),
            count: 0,
            paragraphs: Vec::new(),
            expected: String::new(),
        });
    }
    if let Some(n) = lists.bibliography_entries {
        issues.push(Issue {
            kind: "info",
            group: "lists",
            message: format!("Daftar pustaka: {n} entri"),
            count: n,
            paragraphs: Vec::new(),
            expected: String::new(),
        });
    }

    let to_counted = |map: BTreeMap<String, usize>| {
        map.into_iter()
            .map(|(value, count)| Counted { value, count })
            .collect()
    };

    ScanReport {
        paragraphs: paras.len(),
        tables: doc.tables().len(),
        sections: sections.len(),
        has_header,
        has_footer,
        fonts: to_counted(fonts),
        heading_fonts: to_counted(heading_fonts),
        sizes: to_counted(sizes),
        heading_sizes: to_counted(heading_sizes),
        spacings: to_counted(spacings),
        margins_cm,
        default_font,
        default_size_pt,
        issues,
        lists,
    }
}

/// Perbaiki .docx sesuai preset, kembalikan byte .docx baru.
/// Yang diperbaiki hanya aspek yang diatur profil; aspek yang
/// dikosongkan (None) tidak disentuh.
pub fn fix_docx(bytes: &[u8], preset_name: &str) -> Result<Vec<u8>, String> {
    let profile = preset(preset_name)?;
    fix_with_profile(bytes, &profile)
}

/// Perbaiki .docx sesuai profil kustom (JSON), kembalikan byte .docx baru.
pub fn fix_docx_profile(bytes: &[u8], profile_json: &str) -> Result<Vec<u8>, String> {
    let profile = profile_from_json(profile_json)?;
    fix_with_profile(bytes, &profile)
}

/// Perbaiki .docx sesuai profil kustom (JSON) dan pilihan kelompok
/// aturan (JSON): hanya kelompok yang dicentang yang diterapkan.
pub fn fix_docx_profile_rules(
    bytes: &[u8],
    profile_json: &str,
    rules_json: &str,
) -> Result<Vec<u8>, String> {
    let profile = profile_from_json(profile_json)?;
    let rules = rules_from_json(rules_json)?;
    fix_with_rules(bytes, &profile, &rules)
}

fn fix_with_profile(bytes: &[u8], profile: &Profile) -> Result<Vec<u8>, String> {
    fix_with_rules(bytes, profile, &ApplyRules::legacy())
}

fn fix_with_rules(bytes: &[u8], profile: &Profile, rules: &ApplyRules) -> Result<Vec<u8>, String> {
    let mut doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    // Sampul terdeteksi dari dokumen asli, sebelum perbaikan apa
    // pun mengubah penanda break-nya.
    let xml = document_xml(bytes).unwrap_or_default();
    let cover_end = cover_end_index(&doc, &xml);
    apply_fix(&mut doc, profile, rules, cover_end)?;
    // Penomoran halaman jalan setelah perbaikan format (supaya margin
    // yang disalin ke section baru sudah margin hasil perbaikan),
    // sebelum daftar isi dibuat — break menempel pada paragraf, jadi
    // penyisipan skeleton daftar isi di awal isi tidak menggesernya.
    if rules.page_numbering {
        let bounds = rules.page_bounds.ok_or(
            "Pilih batas penomoran halaman dulu: paragraf terakhir sampul dan paragraf pertama isi.",
        )?;
        apply_page_numbering(&mut doc, &bounds)?;
    }
    let headings = doc
        .paragraphs()
        .iter()
        .filter(|p| is_heading(p.style_id()))
        .count();
    let mut fixed = doc.to_bytes().map_err(|err| err.to_string())?;
    // Kelompok daftar tidak dicentang: dokumen selesai di sini,
    // tidak ada daftar yang dibuatkan atau dibangun ulang.
    if !rules.lists {
        return Ok(fixed);
    }
    // Dokumen punya heading tapi belum ada daftar isi: buatkan
    // skeleton-nya di awal isi, supaya rebuild_toc bisa mengisinya.
    // Daftar tabel/gambar dibuatkan dengan cara yang sama bila ada
    // keterangan berfield SEQ tapi field daftarnya belum ada.
    // (rdocx tidak bisa menyisip field kompleks multi-paragraf lewat
    // API; skeleton disisip lewat bedah zip sebelum dokumen dimuat.)
    let xml = document_xml(&fixed).unwrap_or_default();
    let spans = field_spans(&xml);
    let has_toc = spans.iter().any(|s| matches!(s.kind, FieldKind::Toc));
    let existing_labels: Vec<String> = spans
        .iter()
        .filter(|s| matches!(s.kind, FieldKind::Figures))
        .filter_map(|s| s.label.clone())
        .collect();
    let missing = missing_figure_lists(&seq_caption_labels(&xml), &existing_labels);
    let make_toc = headings > 0 && !has_toc;
    // Lepas dokumen pertama sebelum memuat ulang: dua Document rdocx
    // hidup bersamaan pernah menghabiskan stack thread tes.
    drop(doc);
    if make_toc || !missing.is_empty() {
        fixed = insert_list_skeletons(&fixed, make_toc, has_toc, &missing)?;
    }
    let mut doc = Document::from_bytes(&fixed)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    rebuild_lists(&mut doc)?;
    doc.to_bytes().map_err(|err| err.to_string())
}

/// Buat ulang semua daftar yang sudah ada.
/// Urutan penting: daftar tabel/gambar dulu, baru daftar isi, lalu
/// pustaka. Urutan sebaliknya (daftar isi dulu) pernah gagal di rdocx
/// 0.17 karena rebuild_generated_tables memakai id bookmark yang
/// sudah dipakai rebuild_toc ("duplicate bookmark id").
/// Tidak ada daftar berarti tidak ada yang dibuat ulang (bukan error).
fn rebuild_lists(doc: &mut Document) -> Result<(), String> {
    doc.rebuild_generated_tables()
        .map_err(|err| err.to_string())?;
    doc.rebuild_toc().map_err(|err| err.to_string())?;
    doc.update_bibliography().map_err(|err| err.to_string())?;
    Ok(())
}

/// Skeleton daftar isi: paragraf begin+instr+separate, lalu paragraf
/// end, tanpa entri dan tanpa judul. Struktur paragraf jamak ini yang
/// diminta rebuild_toc ("table of contents result must span paragraph
/// boundaries"); field satu paragraf ditolak.
const TOC_SKELETON: &str = concat!(
    "<w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
    "<w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" \\h \\z \\u </w:instrText></w:r>",
    "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r></w:p>",
    "<w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
);

/// Skeleton satu daftar tabel/gambar: paragraf judul, lalu field
/// kompleks TOC \c dua paragraf (begin+instr+separate, lalu end) —
/// bentuk yang sama dengan skeleton daftar isi. Instruksi meniru
/// yang ditulis Word untuk daftar berlabel ini (lihat fixture
/// daftar.docx): rebuild_generated_tables rdocx mengisinya dari
/// keterangan berfield SEQ berlabel sama, lengkap dengan nomor
/// halaman hasil layout internalnya.
fn figures_skeleton(label: &str, title: &str) -> String {
    format!(
        "<w:p><w:r><w:t>{title}</w:t></w:r></w:p>\
         <w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>\
         <w:r><w:instrText xml:space=\"preserve\"> TOC \\c {} \\p \"\\t\" \\h </w:instrText></w:r>\
         <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r></w:p>\
         <w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
        xml_escape_text(label),
    )
}

/// Escape teks untuk disisipkan sebagai isi elemen XML.
fn xml_escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Posisi sisip di awal isi: tepat sesudah tag <w:body ...>.
fn body_start(xml: &str) -> Result<usize, String> {
    let body = xml.find("<w:body").ok_or("Tag <w:body> tidak ketemu")?;
    let close = xml[body..]
        .find('>')
        .ok_or("Tag <w:body> tidak tertutup")?
        + body;
    Ok(close + 1)
}

/// Posisi tepat sesudah paragraf penutup field daftar isi pertama
/// (paragraf yang memuat fldChar end-nya); None bila field daftar
/// isi tidak ketemu atau bentuknya tidak dikenali. Field sederhana
/// (fldSimple) dicari juga.
fn toc_field_end(xml: &str) -> Option<usize> {
    // Field kompleks: instruksi di elemen <w:instrText>.
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<w:instrText") {
        let tag_start = offset + pos;
        let gt = xml[tag_start..].find('>')? + tag_start;
        let instr_start = gt + 1;
        let close = xml[instr_start..].find("</w:instrText>")? + instr_start;
        let instr = decode_entities(&xml[instr_start..close]);
        if matches!(
            classify_field(&split_field_tokens(&instr)),
            Some((FieldKind::Toc, _))
        ) {
            let end = xml[close..].find("fldCharType=\"end\"")? + close;
            return Some(xml[end..].find("</w:p>")? + end + "</w:p>".len());
        }
        offset = close + "</w:instrText>".len();
    }
    // Field sederhana: instruksi di atribut w:instr <w:fldSimple>.
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<w:fldSimple") {
        let tag_start = offset + pos;
        let gt = xml[tag_start..].find('>')? + tag_start;
        let tag = &xml[tag_start..gt];
        let is_toc = tag
            .find("w:instr=\"")
            .and_then(|start| {
                let value_start = start + "w:instr=\"".len();
                let value_end = tag[value_start..].find('"')? + value_start;
                Some(decode_entities(&tag[value_start..value_end]))
            })
            .map(|instr| {
                matches!(
                    classify_field(&split_field_tokens(&instr)),
                    Some((FieldKind::Toc, _))
                )
            })
            .unwrap_or(false);
        if is_toc {
            let close = xml[gt..].find("</w:fldSimple>")? + gt + "</w:fldSimple>".len();
            return Some(xml[close..].find("</w:p>")? + close + "</w:p>".len());
        }
        offset = gt + 1;
    }
    None
}

/// Sisip skeleton daftar yang belum ada: daftar isi (bila
/// `make_toc`) dan/atau daftar tabel/gambar (`missing`: label SEQ +
/// judul). Daftar tabel/gambar menempel sesudah daftar isi: sesudah
/// skeleton daftar isi yang baru disisip, atau sesudah field daftar
/// isi yang sudah ada; bila tidak ada daftar isi sama sekali,
/// semuanya di awal isi. Semua entri zip disalin; hanya
/// word/document.xml yang diubah.
fn insert_list_skeletons(
    bytes: &[u8],
    make_toc: bool,
    has_toc: bool,
    missing: &[(String, &'static str)],
) -> Result<Vec<u8>, String> {
    let mut xml = document_xml(bytes)?;
    let mut insertion = String::new();
    if make_toc {
        insertion.push_str(TOC_SKELETON);
    }
    for (label, title) in missing {
        insertion.push_str(&figures_skeleton(label, title));
    }
    // Daftar isi yang sudah ada tidak ikut disisip, jadi daftar baru
    // menempel sesudah fieldnya; posisi tidak ketemu -> awal isi.
    let at = if !make_toc && has_toc {
        toc_field_end(&xml).unwrap_or(body_start(&xml)?)
    } else {
        body_start(&xml)?
    };
    xml.insert_str(at, &insertion);
    rewrite_document_xml(bytes, &xml)
}

/// Tulis ulang byte .docx dengan word/document.xml baru; semua
/// entri zip lain disalin apa adanya.
fn rewrite_document_xml(bytes: &[u8], xml: &str) -> Result<Vec<u8>, String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|err| err.to_string())?;
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut out);
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|err| err.to_string())?;
            let name = file
                .name()
                .map_err(|err| err.to_string())?
                .into_owned();
            let mut data = Vec::new();
            file.read_to_end(&mut data).map_err(|err| err.to_string())?;
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            writer
                .start_file(name.clone(), options)
                .map_err(|err| err.to_string())?;
            let content: &[u8] = if name == "word/document.xml" {
                xml.as_bytes()
            } else {
                &data
            };
            writer
                .write_all(content)
                .map_err(|err| err.to_string())?;
        }
        writer.finish().map_err(|err| err.to_string())?;
    }
    Ok(out.into_inner())
}

/// Info paragraf yang dibaca dari sisi baca sebelum perbaikan:
/// (heading, ukuran run, paragraf isi sejati untuk aturan baku,
/// perataan saat ini, terlindungi). Paragraph/Run (tulis) tidak
/// punya getter style_id/size, jadi semua dibaca dulu di sini.
/// Terlindungi = paragraf sampul atau keterangan gambar/tabel:
/// dilewati seluruhnya oleh perbaikan (lihat catatan modul).
type ParaInfo = (bool, Vec<Option<f64>>, bool, Option<Alignment>, bool);

fn para_info_of(
    p: &rdocx::ParagraphRef,
    in_table: bool,
    caption_ids: &HashSet<String>,
    cover: bool,
) -> ParaInfo {
    let runs: Vec<(Option<String>, Option<f64>)> = p
        .runs()
        .map(|r| (r.font_name().map(str::to_string), r.size()))
        .collect();
    // Paragraf isi sejati untuk aturan baku: bukan entri daftar,
    // bukan kosong, bukan judul, bukan di dalam tabel.
    let body = !in_table
        && !is_list_style(p.style_id())
        && !p.text().trim().is_empty()
        && !is_title(&runs);
    let protected = cover || is_caption(p.style_id(), &p.text(), caption_ids);
    (
        is_heading(p.style_id()),
        runs.iter().map(|(_, size)| *size).collect(),
        body,
        p.alignment(),
        protected,
    )
}

fn apply_fix(
    doc: &mut Document,
    profile: &Profile,
    rules: &ApplyRules,
    cover_end: Option<usize>,
) -> Result<(), String> {
    let caption_ids = caption_style_ids(doc);
    // Paragraf isi.
    let para_info: Vec<ParaInfo> = doc
        .paragraphs()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let cover = cover_end.map(|end| i <= end).unwrap_or(false);
            para_info_of(p, false, &caption_ids, cover)
        })
        .collect();
    for (i, (heading, run_sizes, body, alignment, protected)) in para_info.iter().enumerate() {
        if *protected {
            continue;
        }
        if let Some(mut p) = doc.paragraph_mut(i) {
            fix_paragraph(&mut p, profile, rules, *heading, run_sizes, *body, *alignment);
        }
    }
    // Paragraf di sel tabel.
    let tables: Vec<(usize, usize)> = doc
        .tables()
        .iter()
        .map(|t| (t.row_count(), t.column_count()))
        .collect();
    for (ti, (rows, cols)) in tables.iter().enumerate() {
        // Penanda heading dan ukuran run per sel, dibaca dulu.
        let mut flags: Vec<Vec<Vec<ParaInfo>>> = Vec::new();
        if let Some(table_ref) = doc.tables().get(ti) {
            for row in 0..*rows {
                let mut row_flags = Vec::new();
                for col in 0..*cols {
                    let cell_flags = table_ref
                        .cell(row, col)
                        .map(|cell| {
                            cell.paragraphs()
                                .map(|p| para_info_of(&p, true, &caption_ids, false))
                                .collect()
                        })
                        .unwrap_or_default();
                    row_flags.push(cell_flags);
                }
                flags.push(row_flags);
            }
        }
        if let Some(mut table) = doc.table_mut(ti) {
            for row in 0..*rows {
                for col in 0..*cols {
                    if let Some(mut cell) = table.cell(row, col) {
                        let count = cell.paragraphs().count();
                        for pi in 0..count {
                            if let Some(mut p) = cell.paragraph_mut(pi) {
                                let (heading, run_sizes, body, alignment, protected) = flags
                                    .get(row)
                                    .and_then(|r| r.get(col))
                                    .and_then(|c| c.get(pi))
                                    .cloned()
                                    .unwrap_or((false, Vec::new(), false, None, false));
                                if protected {
                                    continue;
                                }
                                fix_paragraph(
                                    &mut p, profile, rules, heading, &run_sizes, body, alignment,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    // Semua section. Hanya sisi margin yang diatur profil yang
    // diubah; sisi lain dipertahankan. Kelompok margin yang tidak
    // dicentang dilewati seluruhnya.
    let target = profile.margins();
    if rules.margins
        && (target.0.is_some() || target.1.is_some() || target.2.is_some() || target.3.is_some())
    {
        let current: Vec<Option<(f32, f32, f32, f32)>> = doc
            .sections()
            .map(|s| {
                s.margins().map(|m| {
                    (
                        m.0.to_cm() as f32,
                        m.1.to_cm() as f32,
                        m.2.to_cm() as f32,
                        m.3.to_cm() as f32,
                    )
                })
            })
            .collect();
        let section_count = current.len();
        for i in 0..section_count {
            if let Some(mut section) = doc.section_mut(i) {
                let now = current[i].unwrap_or((2.54, 2.54, 2.54, 2.54));
                section
                    .set_margins(
                        Length::cm(target.0.unwrap_or(now.0) as f64),
                        Length::cm(target.1.unwrap_or(now.1) as f64),
                        Length::cm(target.2.unwrap_or(now.2) as f64),
                        Length::cm(target.3.unwrap_or(now.3) as f64),
                    )
                    .map_err(|err| err.to_string())?;
            }
        }
    }
    // Font bawaan dokumen, supaya run tanpa format eksplisit ikut berubah.
    if rules.font {
        if let Some(font) = &profile.font {
            doc.set_default_font_name_value(Some(font))
                .map_err(|err| err.to_string())?;
        }
    }
    if rules.size {
        if let Some(size) = profile.size_pt {
            doc.set_default_font_size_value(Some(Length::pt(size)))
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

/// Perbaiki satu paragraf sesuai profil dan pilihan aturan: heading
/// hanya diubah font-nya (ukuran dan spasi dipertahankan); paragraf
/// isi diubah font, ukuran, dan spasi, kecuali run berukuran judul
/// (>= 14 pt) yang ukurannya dipertahankan. Aspek yang tidak diatur
/// profil (None) atau kelompoknya tidak dicentang tidak disentuh
/// sama sekali. Aturan baku (indentasi, perataan, jarak antarparagraf)
/// hanya untuk paragraf isi sejati (`body`): bukan heading, bukan
/// entri daftar, bukan judul, bukan paragraf di dalam tabel.
fn fix_paragraph(
    p: &mut rdocx::Paragraph,
    profile: &Profile,
    rules: &ApplyRules,
    heading: bool,
    run_sizes: &[Option<f64>],
    body: bool,
    alignment: Option<Alignment>,
) {
    if !heading && rules.spacing {
        if let Some(target) = profile.line_spacing {
            p.set_line_spacing_multiple(target);
        }
    }
    if body && !heading {
        if rules.indent {
            p.set_first_line_indent(Length::cm(FIRST_LINE_INDENT_CM));
        }
        // Paragraf yang memang diatur tengah/kanan (keterangan
        // tabel/gambar, judul bab) tidak dipaksa justify.
        if rules.alignment
            && !matches!(alignment, Some(Alignment::Center) | Some(Alignment::Right))
        {
            p.set_alignment(Alignment::Justify);
        }
        if rules.paragraph_spacing {
            p.set_space_before(Length::pt(0.0));
            p.set_space_after(Length::pt(0.0));
        }
    }
    let font_target: Option<&str> = if heading {
        if rules.headings {
            profile.heading_font.as_deref()
        } else {
            None
        }
    } else if rules.font {
        profile.font.as_deref()
    } else {
        None
    };
    let run_count = p.run_count();
    for j in 0..run_count {
        let size_target: Option<f64> = if !heading && rules.size {
            profile.size_pt.filter(|_| {
                !run_sizes
                    .get(j)
                    .copied()
                    .flatten()
                    .map(|size| size >= TITLE_MIN_SIZE_PT)
                    .unwrap_or(false)
            })
        } else {
            None
        };
        let apply = |run: &mut rdocx::Run| {
            if let Some(target) = font_target {
                run.set_font(target);
            }
            if let Some(target) = size_target {
                run.set_size(target);
            }
        };
        if let Some(mut run) = p.run_mut(j) {
            apply(&mut run);
        } else if let Some(path) = p.run_path(j) {
            // Run tersarang (di dalam revisi w:ins, hyperlink, atau
            // content control) tidak bisa disentuh run_mut, yang
            // hanya menerima run langsung — padahal sisi baca
            // (pindai) melihatnya. Tanpa jalur ini, teks hasil
            // sisipan revisi (track changes) terbaca pindai tapi
            // diam-diam terlewat perbaikan. edit_run adalah API
            // rdocx untuk menyunting run pada pemilik rekursifnya.
            let _ = p.edit_run(&path, apply);
        }
    }
}

/// Paragraf footer kosong (sampul): tanpa nomor, tapi tetap satu
/// paragraf supaya part footer valid.
const EMPTY_FOOTER_PARA: &str =
    "<w:p xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"/>";

/// Paragraf footer bernomor: field PAGE kompleks, rata tengah,
/// hasil cache "1" (Word/LibreOffice menghitung ulang saat dibuka).
const PAGE_FOOTER_PARA: &str = concat!(
    "<w:p xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">",
    "<w:pPr><w:jc w:val=\"center\"/></w:pPr>",
    "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
    "<w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r>",
    "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
    "<w:r><w:t>1</w:t></w:r>",
    "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
);

/// Tulis satu paragraf ke footer sebuah section. Paragraf disisip
/// di akhir story lewat `insert_content`, bukan ditimpa: footer
/// baru dari rdocx kosong (tanpa paragraf), dan bila section sudah
/// punya footer (footer pengguna), `create_section_story`
/// mengembalikan story yang sudah ada itu, jadi paragraf baru
/// menempel sesudah isi footer yang ada.
fn insert_footer_paragraph(doc: &mut Document, section: usize, para_xml: &str) -> Result<(), String> {
    let story = doc
        .create_section_story(section, HeaderFooterKind::Footer, HdrFtrType::Default)
        .map_err(|err| err.to_string())?;
    let paragraph =
        CT_P::from_xml_fragment_checked(para_xml.as_bytes()).map_err(|err| err.to_string())?;
    let fragment = ContentFragment::paragraph(paragraph).map_err(|err| err.to_string())?;
    doc.insert_content(&ContentLocation::end(story), fragment)
        .map_err(|err| err.to_string())
}

/// Tulis format nomor halaman (`w:fmt`) ke `w:pgNumType` sebuah
/// section. Format tidak dimodelkan rdocx 0.17 (`CT_PageNumberType`
/// hanya punya `start`), jadi XML sectPr dibaca, atribut fmt
/// disisip (atau nilainya diganti bila sudah ada), lalu ditulis
/// kembali lewat `replace_section_properties_xml`.
fn set_page_number_format(doc: &mut Document, index: usize, fmt: &str) -> Result<(), String> {
    let bytes = doc
        .section_properties_xml(index)
        .map_err(|err| err.to_string())?;
    let mut xml = String::from_utf8(bytes).map_err(|err| err.to_string())?;
    if let Some(tag_start) = xml.find("<w:pgNumType") {
        let after_tag = tag_start + "<w:pgNumType".len();
        let tag_end = xml[after_tag..]
            .find('>')
            .map(|p| p + after_tag)
            .ok_or("Elemen pgNumType tidak tertutup.")?;
        let tag = &xml[tag_start..tag_end];
        if let Some(fmt_pos) = tag.find("w:fmt=\"") {
            let value_start = tag_start + fmt_pos + "w:fmt=\"".len();
            let value_end = xml[value_start..]
                .find('"')
                .map(|p| p + value_start)
                .ok_or("Atribut fmt tidak tertutup.")?;
            xml.replace_range(value_start..value_end, fmt);
        } else {
            xml.insert_str(after_tag, &format!(" w:fmt=\"{fmt}\""));
        }
    } else {
        // Tanpa pgNumType (tidak terjadi setelah set_page_number_start,
        // tapi ditangani): sisip elemen sebelum <w:cols> bila ada,
        // kalau tidak di akhir sectPr.
        let element = format!("<w:pgNumType w:fmt=\"{fmt}\" w:start=\"1\"/>");
        match xml.find("<w:cols") {
            Some(pos) => xml.insert_str(pos, &element),
            None => {
                let end = xml
                    .rfind("</w:sectPr>")
                    .ok_or("Elemen sectPr tidak tertutup.")?;
                xml.insert_str(end, &element);
            }
        }
    }
    doc.replace_section_properties_xml(index, &xml)
        .map_err(|err| err.to_string())
}

/// Terapkan penomoran halaman skema makalah: 3 section, 2 break.
/// Section 1 (sampul) tanpa nomor; section 2 (bagian awal) footer
/// PAGE romawi kecil mulai 1; section 3 (isi) footer PAGE arab
/// mulai 1. Footer bawaan dokumen dipertahankan (lihat bawah).
/// Dipanggil setelah `apply_fix`, jadi geometri yang
/// disalin ke section baru sudah margin hasil perbaikan.
fn apply_page_numbering(doc: &mut Document, bounds: &PageNumbering) -> Result<(), String> {
    let para_count = doc.paragraphs().len();
    if bounds.cover_end >= bounds.content_start {
        return Err(
            "Batas penomoran tidak valid: akhir sampul harus sebelum awal isi.".to_string(),
        );
    }
    if bounds.content_start >= para_count {
        return Err(
            "Batas penomoran tidak valid: awal isi di luar jumlah paragraf dokumen.".to_string(),
        );
    }
    if doc.section_count() != 1 {
        return Err(
            "Penomoran halaman hanya untuk dokumen satu section; dokumen ini sudah punya lebih dari satu section."
                .to_string(),
        );
    }
    // Geometri section berjalan, untuk disalin ke sectPr baru.
    // (sectPr dari set_section_break itu default Letter.)
    let first = doc.sections().next().ok_or("Dokumen tanpa section.")?;
    let (page_w, page_h) = first
        .page_size()
        .ok_or("Ukuran halaman dokumen tidak terbaca.")?;
    let margins = first.margins().ok_or("Margin dokumen tidak terbaca.")?;
    let orientation = first.orientation();
    let hf_dist = first.header_footer_distance();
    for idx in [bounds.cover_end, bounds.content_start - 1] {
        doc.paragraph_mut(idx)
            .ok_or("Paragraf batas tidak ketemu.")?
            .set_section_break(SectionBreak::NextPage);
    }
    if doc.section_count() != 3 {
        return Err("Pembuatan section penomoran gagal.".to_string());
    }
    for i in 0..2 {
        let mut section = doc
            .section_mut(i)
            .ok_or("Section penomoran tidak ketemu.")?;
        section
            .set_page_size(page_w, page_h)
            .map_err(|err| err.to_string())?;
        section
            .set_margins(margins.0, margins.1, margins.2, margins.3)
            .map_err(|err| err.to_string())?;
        if let Some(o) = orientation {
            section.set_orientation(o);
        }
        if let Some((h, f)) = hf_dist {
            section
                .set_header_footer_distance(h, f)
                .map_err(|err| err.to_string())?;
        }
    }
    // Restart nomor: bagian awal dan isi sama-sama mulai dari 1.
    for i in 1..3 {
        doc.section_mut(i)
            .ok_or("Section penomoran tidak ketemu.")?
            .set_page_number_start(1)
            .map_err(|err| err.to_string())?;
    }
    // Footer per section: sampul tanpa nomor, bagian awal + isi
    // bernomor. Dokumen yang sudah punya footer tidak ditolak:
    // setelah break, footer asli pengguna menempel di section isi
    // (sectPr asal menjadi sectPr section penutup), dan
    // `create_section_story` mengembalikan story yang sudah ada di
    // section pemiliknya, jadi paragraf PAGE tersisip sesudah isi
    // footer pengguna — bukan menimpanya (terverifikasi pada
    // fixture uji.docx). Supaya footer pengguna tetap tampil di
    // sampul dan bagian awal seperti sebelum dokumen dipecah,
    // story aslinya disalin ke dua section itu sebagai klon
    // independen (`replace_section_story`; isi part dan relasinya
    // ikut disalin). Sampul hanya menerima salinan itu, tanpa
    // nomor: footer pengguna memang tampil di sampul sebelum
    // penomoran, dan menghapus tampilannya akan mengubah dokumen
    // lebih dari yang diminta penomoran.
    let user_footer = doc
        .section_story(2, HeaderFooterKind::Footer, HdrFtrType::Default)
        .map_err(|err| err.to_string())?
        .filter(|story| !story.is_inherited())
        .map(|story| story.story().clone());
    if let Some(source) = &user_footer {
        for i in 0..2 {
            doc.replace_section_story(i, HeaderFooterKind::Footer, HdrFtrType::Default, source)
                .map_err(|err| err.to_string())?;
        }
    }
    for i in 0..3 {
        if user_footer.is_some() && i == 0 {
            continue;
        }
        let para_xml = if i == 0 {
            EMPTY_FOOTER_PARA
        } else {
            PAGE_FOOTER_PARA
        };
        insert_footer_paragraph(doc, i, para_xml)?;
    }
    set_page_number_format(doc, 1, "lowerRoman")?;
    set_page_number_format(doc, 2, "decimal")?;
    Ok(())
}

/// Doc Format: seluruh teks dokumen (paragraf isi + sel tabel),
/// untuk membandingkan isi sebelum dan sesudah perbaikan.
pub fn docx_text(bytes: &[u8]) -> Result<String, String> {
    let doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    Ok(all_text(&doc))
}

/// Style paragraf entri daftar (hasil Word/rdocx): entri daftar isi
/// (TOC1-TOC9), daftar tabel/gambar (TableofFigures), daftar pustaka
/// (Bibliography). Entri daftar bukan isi yang ditulis pengguna, jadi
/// tidak dihitung dalam banding teks sebelum/sesudah perbaikan.
fn is_list_style(style_id: Option<&str>) -> bool {
    let Some(style) = style_id else {
        return false;
    };
    let lower = style.to_lowercase();
    if lower == "tableoffigures" || lower == "bibliography" {
        return true;
    }
    if let Some(rest) = lower.strip_prefix("toc") {
        return !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// Seluruh teks dokumen (paragraf isi + sel tabel), untuk memastikan
/// perbaikan tidak mengubah teks isi. Paragraf entri daftar dan paragraf
/// kosong dilewati: entri daftar dibuat ulang mesin, paragraf kosong
/// (termasuk paragraf pembuka/penutup field daftar) bukan kata-kata isi.
/// Spasi di dalam paragraf dinormalisasi (rangkaian spasi/tab jadi satu
/// spasi), karena rebuild field bisa menambah spasi di depan hasil
/// sitasi; yang dibandingkan adalah kata-katanya, bukan spasinya.
pub fn all_text(doc: &Document) -> String {
    fn push_part(parts: &mut Vec<String>, p: &rdocx::ParagraphRef) {
        if is_list_style(p.style_id()) {
            return;
        }
        let norm = p
            .text()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !norm.is_empty() {
            parts.push(norm);
        }
    }

    let mut parts = Vec::new();
    for p in doc.paragraphs() {
        push_part(&mut parts, &p);
    }
    for table in doc.tables() {
        for row in 0..table.row_count() {
            for col in 0..table.column_count() {
                if let Some(cell) = table.cell(row, col) {
                    for p in cell.paragraphs() {
                        push_part(&mut parts, &p);
                    }
                }
            }
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).unwrap()
    }

    fn scan_json(bytes: &[u8], preset: &str) -> serde_json::Value {
        let json = scan_docx(bytes, preset).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn issue_kinds(report: &serde_json::Value) -> Vec<String> {
        report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["kind"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn pindai_menemukan_masalah_campuran() {
        let report = scan_json(&fixture("campuran.docx"), "umum");
        // 4 paragraf isi (1 heading + 3) + 3 paragraf sel tabel berisi
        // (sel keempat kosong, tidak dihitung sebagai isi).
        assert_eq!(report["paragraphs"], 7);
        assert_eq!(report["tables"], 1);
        let kinds = issue_kinds(&report);
        assert!(kinds.contains(&"font".to_string()), "kinds: {kinds:?}");
        assert!(kinds.contains(&"size".to_string()));
        assert!(kinds.contains(&"spacing".to_string()));
        assert!(kinds.contains(&"margin".to_string()));
        let margins = report["margins_cm"].as_array().unwrap();
        assert!((margins[0].as_f64().unwrap() - 2.0).abs() < 0.1);
    }

    #[test]
    fn profil_kosong_tidak_mengubah_apa_pun() {
        // Semua kolom dikosongkan: pindai tidak menemukan masalah dan
        // perbaikan tidak mengubah apa pun.
        let profile = profile_from_json(
            r#"{"name":"Kosong","font":null,"size_pt":null,"line_spacing":null,"margin_top_cm":null,"margin_right_cm":null,"margin_bottom_cm":null,"margin_left_cm":null,"heading_font":null}"#,
        )
        .unwrap();
        let original = fixture("campuran.docx");
        let report = scan(&Document::from_bytes(&original).unwrap(), &profile, &original);
        // Kelompok yang diatur profil harus bersih. Aturan baku
        // (indentasi, perataan, jarak antarparagraf) bukan bagian
        // profil, jadi tetap dinilai pindai dan tidak dihitung di sini.
        let actionable: Vec<_> = report
            .issues
            .iter()
            .filter(|i| {
                i.kind != "info"
                    && matches!(i.group, "font" | "size" | "spacing" | "margins" | "headings")
            })
            .collect();
        assert!(actionable.is_empty(), "temuan: {actionable:?}");
        let fixed = fix_with_profile(&original, &profile).unwrap();
        let after = scan(&Document::from_bytes(&fixed).unwrap(), &profile, &fixed);
        assert_eq!(all_text(&Document::from_bytes(&original).unwrap()), all_text(&Document::from_bytes(&fixed).unwrap()));
        let _ = after;
    }

    #[test]
    fn profil_sebagian_hanya_font() {
        // Hanya font yang diatur: pindai hanya menemukan masalah font,
        // perbaikan hanya mengubah font (ukuran/spasi/margin tetap).
        let profile = profile_from_json(
            r#"{"name":"Hanya font","font":"Arial","size_pt":null,"line_spacing":null,"margin_top_cm":null,"margin_right_cm":null,"margin_bottom_cm":null,"margin_left_cm":null,"heading_font":null}"#,
        )
        .unwrap();
        let original = fixture("campuran.docx");
        let report = scan(&Document::from_bytes(&original).unwrap(), &profile, &original);
        let kinds = report
            .issues
            .iter()
            .map(|i| i.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"font"), "kinds: {kinds:?}");
        assert!(!kinds.contains(&"size"), "kinds: {kinds:?}");
        assert!(!kinds.contains(&"spacing"), "kinds: {kinds:?}");
        assert!(!kinds.contains(&"margin"), "kinds: {kinds:?}");
        let fixed = fix_with_profile(&original, &profile).unwrap();
        let after_doc = Document::from_bytes(&fixed).unwrap();
        let after = scan(&after_doc, &profile, &fixed);
        // Kelompok profil bersih; aturan baku tidak diterapkan jalur
        // lama, jadi temuannya boleh tersisa di sini.
        assert!(
            after
                .issues
                .iter()
                .all(|i| i.kind == "info"
                    || matches!(i.group, "indent" | "alignment" | "paragraph_spacing")),
            "masih ada masalah: {:?}",
            after.issues
        );
        // Margin tidak disentuh: tetap 2 cm.
        let margins = after_doc.sections().next().unwrap().margins().unwrap();
        assert!((margins.0.to_cm() - 2.0).abs() < 0.1);
    }

    #[test]
    fn profil_margin_sebagian_hanya_sisi_atas() {
        let profile = profile_from_json(
            r#"{"name":"Margin atas","font":null,"size_pt":null,"line_spacing":null,"margin_top_cm":4.0,"margin_right_cm":null,"margin_bottom_cm":null,"margin_left_cm":null,"heading_font":null}"#,
        )
        .unwrap();
        let original = fixture("campuran.docx");
        let report = scan(&Document::from_bytes(&original).unwrap(), &profile, &original);
        let margin_issue = report
            .issues
            .iter()
            .find(|i| i.kind == "margin")
            .expect("harus ada temuan margin");
        assert!(margin_issue.message.contains("atas"), "pesan: {}", margin_issue.message);
        assert!(!margin_issue.message.contains("kanan"), "pesan: {}", margin_issue.message);
        let fixed = fix_with_profile(&original, &profile).unwrap();
        let after_doc = Document::from_bytes(&fixed).unwrap();
        let margins = after_doc.sections().next().unwrap().margins().unwrap();
        assert!((margins.0.to_cm() - 4.0).abs() < 0.1);
        assert!((margins.1.to_cm() - 2.0).abs() < 0.1);
        assert!((margins.2.to_cm() - 2.0).abs() < 0.1);
        assert!((margins.3.to_cm() - 2.0).abs() < 0.1);
    }

    #[test]
    fn profil_json_tidak_valid_ditolak() {
        assert!(profile_from_json("bukan json").is_err());
        // JSON parsial tetap valid: kolom yang tidak ada berarti None
        // (aspek itu tidak diubah).
        let partial = profile_from_json(r#"{"name":"X"}"#).unwrap();
        assert!(partial.font.is_none());
        assert!(partial.size_pt.is_none());
    }

    #[test]
    fn perbaiki_lalu_pindai_ulang_bersih() {
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let report = scan_json(&fixed, "umum");
        let kinds = issue_kinds(&report);
        assert!(
            !kinds.iter().any(|k| k == "font" || k == "size" || k == "spacing" || k == "margin"),
            "masih ada temuan: {}",
            report["issues"]
        );
    }

    #[test]
    fn font_heading_dalam_bookmark_ikut_diperbaiki() {
        // Penjaga: run heading yang terbungkus bookmark (penanda
        // _Toc bawaan daftar isi Word) terbaca pindai sebagai run
        // heading dan harus ikut diperbaiki.
        let original = fixture("heading-bookmark.docx");
        let profile = profile_from_json(
            r#"{"name":"Arial semua","font":"Arial","size_pt":null,"line_spacing":null,"margin_top_cm":null,"margin_right_cm":null,"margin_bottom_cm":null,"margin_left_cm":null,"heading_font":"Arial"}"#,
        )
        .unwrap();
        let fixed = fix_with_profile(&original, &profile).unwrap();
        let report = scan(&Document::from_bytes(&fixed).unwrap(), &profile, &fixed);
        assert_eq!(report.heading_fonts.len(), 1, "font heading harus tunggal");
        assert_eq!(report.heading_fonts[0].value, "Arial");
    }

    #[test]
    fn font_heading_dalam_pembungkus_apa_pun_ikut_diperbaiki() {
        // Run heading bisa terbungkus elemen lain di dokumen nyata:
        // hyperlink, revisi sisipan (w:ins dari track changes), atau
        // paragraf heading di dalam content control (sdt). Semua
        // terbaca pindai; semua harus ikut diperbaiki. Kasus w:ins
        // adalah bug yang dilaporkan pengguna: font heading yang
        // dipilih tidak diterapkan karena run di dalam revisi hanya
        // bisa disentuh lewat edit_run, bukan run_mut.
        let profile = profile_from_json(
            r#"{"name":"Arial semua","font":"Arial","size_pt":null,"line_spacing":null,"margin_top_cm":null,"margin_right_cm":null,"margin_bottom_cm":null,"margin_left_cm":null,"heading_font":"Arial"}"#,
        )
        .unwrap();
        for name in [
            "heading-hyperlink.docx",
            "heading-ins.docx",
            "heading-sdt.docx",
        ] {
            let original = fixture(name);
            let fixed = fix_with_profile(&original, &profile).unwrap();
            let report = scan(&Document::from_bytes(&fixed).unwrap(), &profile, &fixed);
            assert_eq!(report.heading_fonts.len(), 1, "{name}: font heading harus tunggal");
            assert_eq!(report.heading_fonts[0].value, "Arial", "{name}: font heading tidak berubah");
            assert_eq!(report.heading_fonts[0].count, 1, "{name}: jumlah run heading berubah");
        }
    }

    #[test]
    fn teks_isi_tidak_berubah_setelah_diperbaiki() {
        // Perbaikan hanya mengubah format: teks isi sebelum dan
        // sesudah perbaikan identik. Entri daftar bukan teks isi
        // (dibangun ulang — lihat `perbaiki_membangun_ulang_daftar`).
        let original = fixture("campuran.docx");
        let before = {
            let doc = Document::from_bytes(&original).unwrap();
            all_text(&doc)
        };
        let fixed = fix_docx(&original, "umum").unwrap();
        let after = {
            let doc = Document::from_bytes(&fixed).unwrap();
            all_text(&doc)
        };
        assert_eq!(before, after);
        assert!(before.contains("Paragraf pertama memakai Calibri sebelas."));
        assert!(before.contains("Sel tabel Calibri."));
    }

    #[test]
    fn heading_pertahankan_ukuran() {
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let heading = doc
            .paragraphs()
            .into_iter()
            .find(|p| is_heading(p.style_id()))
            .expect("heading tidak ketemu");
        let run = heading.runs().next().expect("run heading kosong");
        assert_eq!(run.font_name(), Some("Times New Roman"));
        // Ukuran heading 16 pt dipertahankan, bukan diratakan ke 12.
        assert_eq!(run.size(), Some(16.0));
    }

    #[test]
    fn judul_ukuran_besar_dipertahankan() {
        // Run 14 pt di paragraf isi dianggap judul: font diseragamkan,
        // ukuran dipertahankan (tanpa ini, judul sampul makalah mengecil).
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let para = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf kedua"))
            .expect("paragraf kedua tidak ketemu");
        let run = para.runs().next().expect("run kosong");
        assert_eq!(run.font_name(), Some("Times New Roman"));
        assert_eq!(run.size(), Some(14.0));
        // Pindai ulang: ukuran judul hanya jadi catatan, bukan masalah
        // (kelompok profil yang dinilai; aturan baku tidak diterapkan
        // jalur preset lama).
        let report = scan_json(&fixed, "umum");
        let actionable: Vec<_> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| {
                matches!(
                    i["kind"].as_str().unwrap(),
                    "font" | "size" | "spacing" | "margin" | "heading"
                )
            })
            .collect();
        assert!(actionable.is_empty(), "masih ada masalah: {actionable:?}");
    }

    #[test]
    fn sel_tabel_ikut_diperbaiki() {
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        for table in doc.tables() {
            for row in 0..table.row_count() {
                for col in 0..table.column_count() {
                    if let Some(cell) = table.cell(row, col) {
                        for p in cell.paragraphs() {
                            for r in p.runs() {
                                assert_eq!(r.font_name(), Some("Times New Roman"));
                                assert_eq!(r.size(), Some(12.0));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn margin_dan_font_bawaan_diterapkan() {
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let margins = doc.sections().next().unwrap().margins().unwrap();
        assert!((margins.0.to_cm() - 4.0).abs() < 0.1);
        assert!((margins.1.to_cm() - 3.0).abs() < 0.1);
        assert!((margins.2.to_cm() - 3.0).abs() < 0.1);
        assert!((margins.3.to_cm() - 3.0).abs() < 0.1);
        assert_eq!(doc.default_font_name(), Some("Times New Roman"));
        assert!((doc.default_font_size().unwrap().to_pt() - 12.0).abs() < 0.01);
    }

    #[test]
    fn dokumen_uji_juga_bersih_setelah_diperbaiki() {
        // uji.docx punya header/footer/gambar/tabel/list.
        let original = fixture("uji.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let report = scan_json(&fixed, "umum");
        let kinds = issue_kinds(&report);
        assert!(
            !kinds.iter().any(|k| k == "font" || k == "size" || k == "spacing" || k == "margin"),
            "masih ada temuan: {}",
            report["issues"]
        );
        // Info header/footer boleh tetap ada.
        assert!(report["has_header"].as_bool().unwrap());
    }

    #[test]
    fn byte_bukan_docx_ditolak() {
        assert!(scan_docx(b"bukan docx sama sekali", "umum").is_err());
        assert!(fix_docx(b"bukan docx sama sekali", "umum").is_err());
        assert!(scan_docx(&[], "umum").is_err());
    }

    #[test]
    fn preset_tidak_dikenal_ditolak() {
        let original = fixture("campuran.docx");
        assert!(scan_docx(&original, "kampus-lain").is_err());
        assert!(fix_docx(&original, "kampus-lain").is_err());
    }

    #[test]
    fn pindai_melaporkan_status_daftar() {
        // daftar.docx: TOC basi 2 entri, daftar tabel 1, daftar gambar
        // 1, daftar pustaka 1, 4 heading.
        let report = scan_json(&fixture("daftar.docx"), "umum");
        let lists = &report["lists"];
        assert_eq!(lists["toc_entries"], 2);
        assert_eq!(lists["headings"], 4);
        assert_eq!(lists["bibliography_entries"], 1);
        let tables = lists["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 2);
        let tabel = tables.iter().find(|t| t["label"] == "Tabel").unwrap();
        assert_eq!(tabel["entries"], 1);
        let gambar = tables.iter().find(|t| t["label"] == "Gambar").unwrap();
        assert_eq!(gambar["entries"], 1);
        let messages: Vec<_> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["message"].as_str().unwrap().to_string())
            .collect();
        assert!(messages.iter().any(|m| m == "Daftar isi: 2 entri"), "{messages:?}");
        assert!(messages.iter().any(|m| m == "Daftar Tabel: 1 entri"), "{messages:?}");
        assert!(messages.iter().any(|m| m == "Daftar Gambar: 1 entri"), "{messages:?}");
        assert!(messages.iter().any(|m| m == "Daftar pustaka: 1 entri"), "{messages:?}");
    }

    #[test]
    fn perbaiki_membangun_ulang_daftar() {
        let original = fixture("daftar.docx");
        let before_text = all_text(&Document::from_bytes(&original).unwrap());
        assert!(before_text.contains("Bab Satu: Pendahuluan"));
        let fixed = fix_docx(&original, "umum").unwrap();
        // Entri basi hilang, entri baru terisi dari heading (4).
        let report = scan_json(&fixed, "umum");
        assert_eq!(report["lists"]["toc_entries"], 4);
        assert_eq!(report["lists"]["bibliography_entries"], 1);
        let tables = report["lists"]["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 2);
        assert!(tables.iter().all(|t| t["entries"] == 1));
        // Pindai ulang tetap 0 masalah (info tidak dihitung).
        let kinds = issue_kinds(&report);
        assert!(
            !kinds.iter().any(|k| k == "font" || k == "size" || k == "spacing" || k == "margin"),
            "masih ada temuan: {}",
            report["issues"]
        );
        // Teks isi identik: entri daftar (basi maupun baru) bukan isi.
        let after_text = all_text(&Document::from_bytes(&fixed).unwrap());
        assert_eq!(before_text, after_text);
        assert!(!after_text.contains("ENTRI BASI"));
        // Daftar pustaka terisi dari sumber (bukan entri basi).
        let xml = document_xml(&fixed).unwrap();
        assert!(xml.contains("Contoh Buku"));
        assert!(!xml.contains("ENTRI PUSTAKA BASI"));
    }

    #[test]
    fn perbaiki_membuatkan_daftar_isi_bila_belum_ada() {
        // uji.docx punya heading tapi belum ada field daftar isi.
        let original = fixture("uji.docx");
        let before = scan_json(&original, "umum");
        assert_eq!(before["lists"]["toc_entries"], serde_json::Value::Null);
        let headings = before["lists"]["headings"].as_u64().unwrap();
        assert!(headings >= 1, "uji.docx harus punya heading");
        let messages: Vec<_> = before["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["message"].as_str().unwrap().to_string())
            .collect();
        assert!(messages.iter().any(|m| m == "Tidak ada daftar isi, akan dibuatkan"), "{messages:?}");
        let fixed = fix_docx(&original, "umum").unwrap();
        let after = scan_json(&fixed, "umum");
        assert_eq!(after["lists"]["toc_entries"].as_u64().unwrap(), headings);
        // Teks isi identik (entri daftar baru tidak dihitung sebagai isi).
        assert_eq!(
            all_text(&Document::from_bytes(&original).unwrap()),
            all_text(&Document::from_bytes(&fixed).unwrap())
        );
    }

    #[test]
    fn perbaiki_membuatkan_daftar_tabel_dan_gambar_bila_belum_ada() {
        // keterangan.docx: daftar isi basi 1 entri, 1 heading,
        // keterangan SEQ Tabel dan SEQ Gambar, tanpa daftar
        // tabel/gambar.
        let original = fixture("keterangan.docx");
        let before = scan_json(&original, "umum");
        assert_eq!(before["lists"]["toc_entries"], 1);
        assert!(before["lists"]["tables"].as_array().unwrap().is_empty());
        let messages: Vec<_> = before["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["message"].as_str().unwrap().to_string())
            .collect();
        assert!(
            messages.iter().any(|m| m == "Tidak ada daftar tabel, akan dibuatkan"),
            "{messages:?}"
        );
        assert!(
            messages.iter().any(|m| m == "Tidak ada daftar gambar, akan dibuatkan"),
            "{messages:?}"
        );
        let fixed = fix_docx(&original, "umum").unwrap();
        let after = scan_json(&fixed, "umum");
        assert_eq!(after["lists"]["toc_entries"], 1);
        let tables = after["lists"]["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 2);
        let tabel = tables.iter().find(|t| t["label"] == "Tabel").unwrap();
        assert_eq!(tabel["entries"], 1);
        let gambar = tables.iter().find(|t| t["label"] == "Gambar").unwrap();
        assert_eq!(gambar["entries"], 1);
        // Urutan di dokumen: daftar tabel dan gambar sesudah field
        // daftar isi, daftar tabel sebelum daftar gambar.
        let xml = document_xml(&fixed).unwrap();
        let toc_at = xml.find("TOC \\o").unwrap();
        let tabel_at = xml.find("<w:t>Daftar Tabel</w:t>").unwrap();
        let gambar_at = xml.find("<w:t>Daftar Gambar</w:t>").unwrap();
        assert!(toc_at < tabel_at && tabel_at < gambar_at);
        // Entri terisi dari keterangan, bukan teks kosong.
        assert!(xml.contains("Contoh data"));
        assert!(xml.contains("Contoh gambar"));
        // Teks isi tidak berubah selain dua paragraf judul daftar.
        let before_text = all_text(&Document::from_bytes(&original).unwrap());
        let after_text = all_text(&Document::from_bytes(&fixed).unwrap());
        let stripped: Vec<&str> = after_text
            .lines()
            .filter(|l| *l != "Daftar Tabel" && *l != "Daftar Gambar")
            .collect();
        assert_eq!(stripped.join("\n"), before_text);
        // Perbaiki hasil perbaikan: daftar tidak dobel, entri tetap.
        let refixed = fix_docx(&fixed, "umum").unwrap();
        let reafter = scan_json(&refixed, "umum");
        let retables = reafter["lists"]["tables"].as_array().unwrap();
        assert_eq!(retables.len(), 2);
        assert!(retables.iter().all(|t| t["entries"] == 1));
    }

    #[test]
    fn keterangan_teks_saja_tidak_dibuatkan_daftar() {
        // keterangan-teks.docx: keterangan diketik manual ("Tabel 1
        // ..." sebagai teks biasa) tanpa field SEQ. Daftar tidak
        // bisa mengumpulkannya (sama seperti Word), jadi tidak
        // dibuatkan dan pindai tidak menjanjikannya.
        let original = fixture("keterangan-teks.docx");
        let before = scan_json(&original, "umum");
        let messages: Vec<_> = before["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["message"].as_str().unwrap().to_string())
            .collect();
        assert!(
            !messages.iter().any(|m| m == "Tidak ada daftar tabel, akan dibuatkan"),
            "{messages:?}"
        );
        assert!(
            !messages.iter().any(|m| m == "Tidak ada daftar gambar, akan dibuatkan"),
            "{messages:?}"
        );
        let fixed = fix_docx(&original, "umum").unwrap();
        let after = scan_json(&fixed, "umum");
        assert!(after["lists"]["tables"].as_array().unwrap().is_empty());
        let xml = document_xml(&fixed).unwrap();
        assert!(!xml.contains("<w:t>Daftar Tabel</w:t>"));
        assert!(!xml.contains("<w:t>Daftar Gambar</w:t>"));
    }

    #[test]
    fn daftar_tabel_gambar_yang_sudah_ada_tidak_diduplikat() {
        // daftar.docx sudah punya daftar tabel dan gambar: perbaikan
        // hanya membangun ulang, tidak menyisip daftar kedua.
        let original = fixture("daftar.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let xml = document_xml(&fixed).unwrap();
        let figures = field_spans(&xml)
            .iter()
            .filter(|s| matches!(s.kind, FieldKind::Figures))
            .count();
        assert_eq!(figures, 2);
        // Judul sisipan akan tampak sebagai teks persis ini; fixture
        // memakai judul kapital dari "dokumen aslinya".
        assert!(!xml.contains("<w:t>Daftar Tabel</w:t>"));
        assert!(!xml.contains("<w:t>Daftar Gambar</w:t>"));
    }

    #[test]
    fn dokumen_tanpa_daftar_tetap_diperbaiki() {
        // campuran.docx: tanpa daftar apa pun, 1 heading -> dibuatkan
        // daftar isi 1 entri; perbaikan format tetap jalan.
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let report = scan_json(&fixed, "umum");
        assert_eq!(report["lists"]["toc_entries"], 1);
        assert_eq!(report["lists"]["bibliography_entries"], serde_json::Value::Null);
        assert!(report["lists"]["tables"].as_array().unwrap().is_empty());
    }

    #[test]
    fn preset_makalah_margin_kiri_empat() {
        let profile = preset("makalah").unwrap();
        assert_eq!(profile.name, "Makalah");
        assert_eq!(profile.font.as_deref(), Some("Times New Roman"));
        assert_eq!(profile.size_pt, Some(12.0));
        assert_eq!(profile.line_spacing, Some(1.5));
        assert_eq!(profile.margin_top_cm, Some(4.0));
        assert_eq!(profile.margin_right_cm, Some(3.0));
        assert_eq!(profile.margin_bottom_cm, Some(3.0));
        assert_eq!(profile.margin_left_cm, Some(4.0));
        // Pindai menemukan margin kiri fixture (2 cm) menyimpang.
        let report = scan_json(&fixture("campuran.docx"), "makalah");
        let margin_issue = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["kind"] == "margin")
            .expect("harus ada temuan margin");
        assert_eq!(margin_issue["group"], "margins");
        assert!(
            margin_issue["message"].as_str().unwrap().contains("kiri"),
            "pesan: {}",
            margin_issue["message"]
        );
        // Perbaikan menerapkan margin makalah persis.
        let fixed = fix_docx(&fixture("campuran.docx"), "makalah").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let margins = doc.sections().next().unwrap().margins().unwrap();
        assert!((margins.0.to_cm() - 4.0).abs() < 0.1);
        assert!((margins.1.to_cm() - 3.0).abs() < 0.1);
        assert!((margins.2.to_cm() - 3.0).abs() < 0.1);
        assert!((margins.3.to_cm() - 4.0).abs() < 0.1);
    }

    #[test]
    fn pindai_melaporkan_kelompok_aturan_baku() {
        // Setiap temuan membawa kelompok; paragraf isi fixture belum
        // berindentasi dan belum justify, jadi kedua kelompok muncul.
        let report = scan_json(&fixture("campuran.docx"), "umum");
        let issues = report["issues"].as_array().unwrap();
        assert!(issues.iter().all(|i| i["group"].is_string()), "{issues:?}");
        let groups: Vec<&str> = issues.iter().map(|i| i["group"].as_str().unwrap()).collect();
        assert!(groups.contains(&"indent"), "groups: {groups:?}");
        assert!(groups.contains(&"alignment"), "groups: {groups:?}");
        assert!(groups.contains(&"font"), "groups: {groups:?}");
        assert!(groups.contains(&"margins"), "groups: {groups:?}");
        assert!(groups.contains(&"lists"), "groups: {groups:?}");
    }

    #[test]
    fn ringkasan_font_dan_ukuran_mengikuti_rincian() {
        // campuran.docx: heading Calibri Light 16 pt (1 run); isi:
        // Calibri 11 pt (2 run), Arial 14 pt judul (1 run), tanpa
        // font/ukuran eksplisit (3 run). Ringkasan memisah isi dan
        // heading, supaya angkanya sama dengan bullet rincian —
        // dulu ringkasan mencampur keduanya (mis. "Arial 3" di
        // ringkasan tapi "Arial 1" di bullet).
        let report = scan_json(&fixture("campuran.docx"), "umum");
        let counted = |key: &str| -> BTreeMap<String, usize> {
            report[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    (
                        c["value"].as_str().unwrap().to_string(),
                        c["count"].as_u64().unwrap() as usize,
                    )
                })
                .collect()
        };
        assert_eq!(
            counted("fonts"),
            BTreeMap::from([
                ("Arial".to_string(), 2),
                ("Bawaan".to_string(), 2),
                ("Calibri".to_string(), 2),
            ])
        );
        assert_eq!(
            counted("heading_fonts"),
            BTreeMap::from([("Calibri Light".to_string(), 1)])
        );
        assert_eq!(
            counted("sizes"),
            BTreeMap::from([
                ("11 pt".to_string(), 2),
                ("14 pt".to_string(), 1),
                ("Bawaan".to_string(), 3),
            ])
        );
        assert_eq!(
            counted("heading_sizes"),
            BTreeMap::from([("16 pt".to_string(), 1)])
        );
        // Bullet rincian memakai angka yang sama dengan ringkasan.
        let messages: Vec<_> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["message"].as_str().unwrap().to_string())
            .collect();
        assert!(
            messages
                .iter()
                .any(|m| m == "Paragraf 2, 5: font Calibri — seharusnya Times New Roman."),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m == "Paragraf 3, 6: font Arial — seharusnya Times New Roman."),
            "{messages:?}"
        );
        assert!(
            messages.iter().any(
                |m| m == "Paragraf 1: heading memakai font Calibri Light — seharusnya Times New Roman."
            ),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m == "Paragraf 2, 5: ukuran 11 pt — seharusnya 12 pt."),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m == "Paragraf 3: ukuran 14 pt dipertahankan sebagai judul."),
            "{messages:?}"
        );
        // Temuan membawa indeks paragraf dan nilai target sebagai
        // data terpisah, bukan hanya di dalam teks pesan. Indeks
        // mengikuti urutan paragraf terpindai: 1 = heading, 2 = isi
        // Calibri, 3 = judul Arial, 4 = isi tanpa font, 5-7 = sel
        // tabel.
        let issues = report["issues"].as_array().unwrap();
        let calibri = issues
            .iter()
            .find(|i| i["kind"] == "font" && i["message"].as_str().unwrap().contains("Calibri"))
            .expect("temuan font Calibri harus ada");
        assert_eq!(calibri["count"], 2);
        assert_eq!(calibri["paragraphs"], serde_json::json!([2, 5]));
        assert_eq!(calibri["expected"], "Times New Roman");
        let title = issues
            .iter()
            .find(|i| i["kind"] == "info" && i["group"] == "size")
            .expect("info judul harus ada");
        assert_eq!(title["paragraphs"], serde_json::json!([3]));
        assert_eq!(title["expected"], "");
        // Temuan margin tidak terikat paragraf: daftarnya kosong,
        // targetnya sisi yang menyimpang.
        let margin = issues
            .iter()
            .find(|i| i["kind"] == "margin")
            .expect("temuan margin harus ada");
        assert_eq!(margin["paragraphs"], serde_json::json!([]));
        // Margin fixture 2 cm di semua sisi; target umum 4-3-3-3.
        assert_eq!(
            margin["message"],
            "Margin atas 2,0 cm — seharusnya 4,0 cm; margin kanan 2,0 cm — seharusnya 3,0 cm; margin bawah 2,0 cm — seharusnya 3,0 cm; margin kiri 2,0 cm — seharusnya 3,0 cm."
        );
        assert_eq!(
            margin["expected"],
            "atas 4,0 cm, kanan 3,0 cm, bawah 3,0 cm, kiri 3,0 cm"
        );
    }

    #[test]
    fn aturan_baku_diterapkan_ke_paragraf_isi() {
        let original = fixture("campuran.docx");
        let profile = builtin_profile();
        let fixed = fix_with_rules(&original, &profile, &ApplyRules::default()).unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let body = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf pertama"))
            .expect("paragraf isi tidak ketemu");
        let indent = body.first_line_indent().expect("indentasi tidak diterapkan");
        assert!((indent.to_cm() - FIRST_LINE_INDENT_CM).abs() < 0.05);
        assert_eq!(body.alignment(), Some(Alignment::Justify));
        assert_eq!(body.space_before().map(|l| l.to_pt()), Some(0.0));
        assert_eq!(body.space_after().map(|l| l.to_pt()), Some(0.0));
        // Paragraf judul (run 14 pt) tidak dipaksa justify/indentasi.
        let title = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf kedua"))
            .expect("paragraf judul tidak ketemu");
        assert_ne!(title.alignment(), Some(Alignment::Justify));
        assert!(title.first_line_indent().is_none());
        // Pindai ulang: kelompok aturan baku bersih.
        let report = scan(&Document::from_bytes(&fixed).unwrap(), &profile, &fixed);
        assert!(
            report
                .issues
                .iter()
                .all(|i| !matches!(i.group, "indent" | "alignment" | "paragraph_spacing")),
            "masih ada temuan aturan baku: {:?}",
            report.issues
        );
        // Isi tetap identik kata per kata.
        assert_eq!(
            all_text(&Document::from_bytes(&original).unwrap()),
            all_text(&Document::from_bytes(&fixed).unwrap())
        );
    }

    #[test]
    fn aturan_baku_dimatikan_tidak_diterapkan() {
        let original = fixture("campuran.docx");
        let profile = builtin_profile();
        let rules = ApplyRules {
            indent: false,
            alignment: false,
            paragraph_spacing: false,
            ..ApplyRules::default()
        };
        let fixed = fix_with_rules(&original, &profile, &rules).unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let body = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf pertama"))
            .expect("paragraf isi tidak ketemu");
        assert!(body.first_line_indent().is_none());
        assert_ne!(body.alignment(), Some(Alignment::Justify));
        assert!(body.space_before().is_none());
        assert!(body.space_after().is_none());
        // Kelompok lain tetap jalan: font sudah Times New Roman.
        let run = body.runs().next().expect("run kosong");
        assert_eq!(run.font_name(), Some("Times New Roman"));
    }

    #[test]
    fn jalur_lama_tidak_menerapkan_aturan_baku() {
        // fix_docx / fix_docx_profile (tanpa pilihan aturan) berperilaku
        // persis seperti sebelum ada pilihan: aturan baku tidak ikut.
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let body = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf pertama"))
            .expect("paragraf isi tidak ketemu");
        assert!(body.first_line_indent().is_none());
        assert_ne!(body.alignment(), Some(Alignment::Justify));
    }

    #[test]
    fn pilihan_aturan_sebagian_lewat_json() {
        // Hanya margin yang dimatikan: font tetap diperbaiki, margin
        // tetap 2 cm, daftar tidak dibuatkan (lists mati).
        let original = fixture("campuran.docx");
        let fixed = fix_docx_profile_rules(
            &original,
            r#"{"name":"Standar Umum","font":"Times New Roman","size_pt":12.0,"line_spacing":1.5,"margin_top_cm":4.0,"margin_right_cm":3.0,"margin_bottom_cm":3.0,"margin_left_cm":3.0,"heading_font":"Times New Roman"}"#,
            r#"{"margins":false,"lists":false}"#,
        )
        .unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let margins = doc.sections().next().unwrap().margins().unwrap();
        assert!((margins.0.to_cm() - 2.0).abs() < 0.1);
        let report = scan_json(&fixed, "umum");
        assert_eq!(report["lists"]["toc_entries"], serde_json::Value::Null);
        // JSON aturan kosong berarti semua kelompok diterapkan.
        let rules = rules_from_json("{}").unwrap();
        assert!(rules.margins && rules.font && rules.indent && rules.lists);
        assert!(rules_from_json("bukan json").is_err());
    }

    /// Dokumen mirip makalah untuk tes penomoran: 3 paragraf sampul,
    /// 3 bagian awal, 4 isi (2 heading), A4 + margin 4/3/3/4.
    fn makalah_doc() -> Vec<u8> {
        let mut doc = Document::new();
        doc.set_page_size(Length::cm(21.0), Length::cm(29.7));
        doc.set_margins(
            Length::cm(4.0),
            Length::cm(3.0),
            Length::cm(3.0),
            Length::cm(4.0),
        );
        doc.add_paragraph("JUDUL MAKALAH");
        doc.add_paragraph("Nama Mahasiswa");
        doc.add_paragraph("NIM 241011401234");
        doc.add_paragraph("KATA PENGANTAR");
        doc.add_paragraph("Kata pengantar isi.");
        doc.add_paragraph("DAFTAR ISI");
        doc.add_paragraph("BAB I PENDAHULUAN").style("Heading1");
        doc.add_paragraph("Isi bab satu.");
        doc.add_paragraph("BAB II PEMBAHASAN").style("Heading1");
        doc.add_paragraph("Isi bab dua.");
        doc.to_bytes().unwrap()
    }

    const PROFILE_JSON: &str = r#"{"name":"Standar Umum","font":"Times New Roman","size_pt":12.0,"line_spacing":1.5,"margin_top_cm":4.0,"margin_right_cm":3.0,"margin_bottom_cm":3.0,"margin_left_cm":3.0,"heading_font":"Times New Roman"}"#;

    /// Isi satu part zip sebagai string (untuk memeriksa XML hasil).
    fn zip_part(bytes: &[u8], name: &str) -> String {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut file = archive.by_name(name).unwrap();
        let mut xml = String::new();
        file.read_to_string(&mut xml).unwrap();
        xml
    }

    // Untuk tiap footerReference (urut section): true bila footer itu
    // berisi field PAGE yang lengkap — fldChar begin, instruksi
    // ` PAGE `, fldChar separate, hasil cache, fldChar end. Cek
    // keberadaan kata "PAGE" saja tidak cukup: field yang rusak
    // (mis. instruksi tanpa begin/end) tetap mengandung kata itu
    // tapi Word tidak menampilkan nomor.
    fn footer_page_flags(bytes: &[u8]) -> Vec<bool> {
        let xml = document_xml(bytes).unwrap();
        let rels = zip_part(bytes, "word/_rels/document.xml.rels");
        let mut flags = Vec::new();
        let mut rest = xml.as_str();
        while let Some(pos) = rest.find("footerReference") {
            let after = &rest[pos..];
            let id_start = after.find("r:id=\"").unwrap() + "r:id=\"".len();
            let id_end = after[id_start..].find('"').unwrap() + id_start;
            let rid = &after[id_start..id_end];
            let rel_pos = rels.find(&format!("Id=\"{rid}\"")).expect("relasi footer harus ada");
            let rel_after = &rels[rel_pos..];
            let tgt_start = rel_after.find("Target=\"").unwrap() + "Target=\"".len();
            let tgt_end = rel_after[tgt_start..].find('"').unwrap() + tgt_start;
            let target = &rel_after[tgt_start..tgt_end];
            let footer = zip_part(bytes, &format!("word/{target}"));
            flags.push(
                footer.contains("w:fldCharType=\"begin\"")
                    && footer.contains(" PAGE ")
                    && footer.contains("w:fldCharType=\"separate\"")
                    && footer.contains("w:fldCharType=\"end\""),
            );
            rest = &rest[pos + "footerReference".len()..];
        }
        flags
    }

    #[test]
    fn penomoran_halaman_tiga_section() {
        let original = makalah_doc();
        let before = all_text(&Document::from_bytes(&original).unwrap());
        let fixed = fix_docx_profile_rules(
            &original,
            PROFILE_JSON,
            r#"{"page_numbering":true,"page_bounds":{"cover_end":2,"content_start":6}}"#,
        )
        .unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        assert_eq!(doc.section_count(), 3);
        // Geometri: semua section tetap A4 + margin hasil perbaikan
        // (profil Standar Umum: 4/3/3/3 — kiri dokumen 4 cm ikut
        // jadi 3 cm; sectPr baru dari set_section_break itu default
        // Letter, jadi ini mengunci penyalinan geometri).
        for (i, section) in doc.sections().enumerate() {
            let (w, h) = section.page_size().unwrap();
            assert!((w.to_cm() - 21.0).abs() < 0.05, "section {i} lebar: {}", w.to_cm());
            assert!((h.to_cm() - 29.7).abs() < 0.05, "section {i} tinggi: {}", h.to_cm());
            let m = section.margins().unwrap();
            for (a, b) in [
                (m.0.to_cm(), 4.0),
                (m.1.to_cm(), 3.0),
                (m.2.to_cm(), 3.0),
                (m.3.to_cm(), 3.0),
            ] {
                assert!((a - b).abs() < 0.05, "section {i} margin: {a} vs {b}");
            }
        }
        // Format nomor: sampul tanpa pgNumType, bagian awal romawi,
        // isi arab, sama-sama mulai dari 1.
        let xml0 = String::from_utf8(doc.section_properties_xml(0).unwrap()).unwrap();
        assert!(!xml0.contains("pgNumType"), "sampul tanpa pgNumType: {xml0}");
        let xml1 = String::from_utf8(doc.section_properties_xml(1).unwrap()).unwrap();
        assert!(xml1.contains("w:fmt=\"lowerRoman\""), "romawi: {xml1}");
        assert!(xml1.contains("w:start=\"1\""), "start: {xml1}");
        let xml2 = String::from_utf8(doc.section_properties_xml(2).unwrap()).unwrap();
        assert!(xml2.contains("w:fmt=\"decimal\""), "arab: {xml2}");
        assert!(xml2.contains("w:start=\"1\""), "start: {xml2}");
        // Footer: sampul tanpa PAGE, bagian awal + isi berisi PAGE.
        assert_eq!(footer_page_flags(&fixed), vec![false, true, true]);
        assert!(doc.validate_sections().is_valid());
        // Teks isi utuh; daftar isi ikut dibuatkan dari 2 heading.
        assert_eq!(before, all_text(&doc));
        let report = scan_json(&fixed, "umum");
        assert_eq!(report["sections"], 3);
        assert_eq!(report["lists"]["toc_entries"], 2);
    }

    #[test]
    fn penomoran_tanpa_batas_ditolak() {
        let original = makalah_doc();
        let err = fix_docx_profile_rules(&original, PROFILE_JSON, r#"{"page_numbering":true}"#)
            .expect_err("tanpa batas harus ditolak");
        assert!(err.contains("Pilih batas"), "pesan: {err}");
        // Batas terbalik / di luar dokumen juga ditolak.
        let err = fix_docx_profile_rules(
            &original,
            PROFILE_JSON,
            r#"{"page_numbering":true,"page_bounds":{"cover_end":6,"content_start":6}}"#,
        )
        .expect_err("batas terbalik harus ditolak");
        assert!(err.contains("tidak valid"), "pesan: {err}");
        let err = fix_docx_profile_rules(
            &original,
            PROFILE_JSON,
            r#"{"page_numbering":true,"page_bounds":{"cover_end":2,"content_start":99}}"#,
        )
        .expect_err("batas di luar dokumen harus ditolak");
        assert!(err.contains("tidak valid"), "pesan: {err}");
    }

    #[test]
    fn penomoran_jalur_lama_tidak_menerapkan() {
        // fix_docx / fix_docx_profile (tanpa pilihan aturan) tidak
        // menyentuh penomoran: tetap 1 section, tanpa footer.
        let original = makalah_doc();
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        assert_eq!(doc.section_count(), 1);
        let names = {
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&fixed)).unwrap();
            (0..archive.len())
                .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert!(
            !names.iter().any(|n| n.starts_with("word/footer")),
            "tidak boleh ada footer: {names:?}"
        );
    }

    #[test]
    fn penomoran_dengan_daftar_isi_yang_dibangun_ulang() {
        // daftar.docx: 4 heading + TOC basi 2 entri, tanpa footer.
        // Penomoran jalan bersama pembuatan ulang daftar: section
        // tetap 3, TOC terisi 4, footer PAGE tetap utuh.
        let original = fixture("daftar.docx");
        let probe = Document::from_bytes(&original).unwrap();
        let content_start = probe
            .paragraphs()
            .iter()
            .position(|p| is_heading(p.style_id()))
            .expect("fixture harus punya heading");
        let fixed = fix_docx_profile_rules(
            &original,
            PROFILE_JSON,
            &format!(
                "{{\"page_numbering\":true,\"page_bounds\":{{\"cover_end\":0,\"content_start\":{content_start}}}}}"
            ),
        )
        .unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        assert_eq!(doc.section_count(), 3);
        assert_eq!(footer_page_flags(&fixed), vec![false, true, true]);
        let report = scan_json(&fixed, "umum");
        assert_eq!(report["lists"]["toc_entries"], 4);
        assert_eq!(
            all_text(&Document::from_bytes(&original).unwrap()),
            all_text(&doc)
        );
    }

    #[test]
    fn penomoran_dokumen_berfooter_isi_footer_utuh() {
        // uji.docx sudah punya footer ("Footer Dokumen Uji"):
        // penomoran tetap jalan, teks footer pengguna utuh di
        // ketiga section (asli di isi, salinan di sampul dan
        // bagian awal), dan di section bernomor paragraf PAGE
        // berdiri sesudah teks itu — disisip, bukan menimpa.
        let original = fixture("uji.docx");
        let fixed = fix_docx_profile_rules(
            &original,
            PROFILE_JSON,
            r#"{"page_numbering":true,"page_bounds":{"cover_end":0,"content_start":2}}"#,
        )
        .unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        assert_eq!(doc.section_count(), 3);
        // Footer: sampul tanpa PAGE, bagian awal + isi berisi PAGE.
        assert_eq!(footer_page_flags(&fixed), vec![false, true, true]);
        // Tepat tiga part footer, semuanya memuat teks pengguna;
        // pada part bernomor, teks pengguna mendahului field PAGE.
        let footer_names: Vec<String> = {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&fixed)).unwrap();
            (0..archive.len())
                .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
                .filter(|n| n.starts_with("word/footer"))
                .collect()
        };
        assert_eq!(footer_names.len(), 3, "footer parts: {footer_names:?}");
        for name in &footer_names {
            let xml = zip_part(&fixed, name);
            let text_pos = xml
                .find("Footer Dokumen Uji")
                .unwrap_or_else(|| panic!("{name} kehilangan footer pengguna: {xml}"));
            if let Some(page_pos) = xml.find(" PAGE ") {
                assert!(text_pos < page_pos, "{name}: PAGE harus sesudah footer pengguna");
            }
        }
        assert!(doc.validate_sections().is_valid());
        // Teks isi utuh.
        assert_eq!(
            all_text(&Document::from_bytes(&original).unwrap()),
            all_text(&doc)
        );
    }

    #[test]
    fn daftar_paragraf_untuk_pemilih_batas() {
        let original = makalah_doc();
        let list: serde_json::Value =
            serde_json::from_str(&docx_paragraphs(&original).unwrap()).unwrap();
        let items = list.as_array().unwrap();
        assert_eq!(items.len(), 10);
        assert_eq!(items[0]["index"], 0);
        assert_eq!(items[0]["text"], "JUDUL MAKALAH");
        assert_eq!(items[6]["text"], "BAB I PENDAHULUAN");
        assert!(docx_paragraphs(b"bukan docx").is_err());
    }

    #[test]
    fn sampul_dan_keterangan_tidak_diubah() {
        let original = fixture("sampul.docx");
        let profile = makalah_profile();
        let fixed = fix_with_rules(&original, &profile, &ApplyRules::default()).unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let paras = doc.paragraphs();
        let find = |needle: &str| {
            paras
                .iter()
                .find(|p| p.text().contains(needle))
                .unwrap_or_else(|| panic!("paragraf {needle:?} tidak ketemu"))
        };
        // Sampul: judul Arial 16 bold tengah spasi 1,0 tetap persis.
        let title = find("JUDUL SKRIPSI CONTOH");
        let run = title.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Arial"));
        assert_eq!(run.size(), Some(16.0));
        assert!(run.is_bold());
        assert_eq!(title.alignment(), Some(Alignment::Center));
        assert_eq!(title.line_spacing_multiple(), Some(1.0));
        // Baris sampul lain juga tidak disentuh.
        let name = find("Nama Mahasiswa Contoh");
        let run = name.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Arial"));
        assert_eq!(run.size(), Some(12.0));
        assert_eq!(name.alignment(), Some(Alignment::Center));
        assert_eq!(name.line_spacing_multiple(), Some(1.0));
        // Isi sesudah sampul DIPERBAIKI seperti biasa.
        let body = find("Paragraf isi pertama");
        let run = body.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Times New Roman"));
        assert_eq!(run.size(), Some(12.0));
        assert_eq!(body.alignment(), Some(Alignment::Justify));
        let indent = body.first_line_indent().expect("isi harus terindentasi");
        assert!((indent.to_cm() - FIRST_LINE_INDENT_CM).abs() < 0.05);
        assert_eq!(body.line_spacing_multiple(), Some(1.5));
        // Keterangan bergaya Caption: Calibri 10 miring tengah tetap.
        let cap = find("Gambar 1.1 Diagram alir");
        let run = cap.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Calibri"));
        assert_eq!(run.size(), Some(10.0));
        assert!(run.is_italic());
        assert_eq!(cap.alignment(), Some(Alignment::Center));
        assert!(cap.first_line_indent().is_none());
        // Keterangan dari pola teks saja (tanpa style Caption):
        // tidak di-justify, tidak diindentasi, ukuran tetap.
        let cap2 = find("Gambar 2.1 Contoh diagram");
        let run = cap2.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Calibri"));
        assert_eq!(run.size(), Some(10.0));
        assert!(run.is_italic());
        assert_ne!(cap2.alignment(), Some(Alignment::Justify));
        assert!(cap2.first_line_indent().is_none());
        // Isi kedua (sesudah keterangan) juga diperbaiki.
        let body2 = find("Paragraf isi kedua");
        assert_eq!(body2.alignment(), Some(Alignment::Justify));
        // Teks utuh.
        assert_eq!(
            all_text(&Document::from_bytes(&original).unwrap()),
            all_text(&doc)
        );
    }

    #[test]
    fn pindai_melaporkan_sampul_dan_keterangan_sebagai_info() {
        let report = scan_json(&fixture("sampul.docx"), "makalah");
        let messages: Vec<_> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["message"].as_str().unwrap().to_string())
            .collect();
        assert!(
            messages.iter().any(|m| m == "Sampul tidak diubah."),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|m| m == "Keterangan gambar/tabel tidak diubah."),
            "{messages:?}"
        );
        // Format khas sampul/keterangan tidak dihitung sebagai
        // temuan: tidak ada temuan font Arial atau ukuran 10 pt.
        assert!(!messages.iter().any(|m| m.contains("Arial")), "{messages:?}");
        assert!(!messages.iter().any(|m| m.contains("10 pt")), "{messages:?}");
        // Sesudah perbaikan, yang tersisa hanya temuan info.
        let profile = makalah_profile();
        let fixed =
            fix_with_rules(&fixture("sampul.docx"), &profile, &ApplyRules::default()).unwrap();
        let after = scan_json(&fixed, "makalah");
        for issue in after["issues"].as_array().unwrap() {
            assert_eq!(issue["kind"], "info", "masih ada temuan: {issue}");
        }
    }

    #[test]
    fn sampul_dari_pagebreakbefore_terdeteksi() {
        let original = fixture("sampul_pbb.docx");
        let profile = makalah_profile();
        let fixed = fix_with_rules(&original, &profile, &ApplyRules::default()).unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let title = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("JUDUL SKRIPSI"))
            .expect("judul sampul tidak ketemu");
        let run = title.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Arial"));
        assert_eq!(run.size(), Some(16.0));
        assert_eq!(title.alignment(), Some(Alignment::Center));
        let body = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf isi pertama"))
            .expect("paragraf isi tidak ketemu");
        let run = body.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Times New Roman"));
        assert_eq!(body.alignment(), Some(Alignment::Justify));
    }

    #[test]
    fn sampul_dari_section_break_terdeteksi() {
        let original = fixture("sampul_sect.docx");
        let profile = makalah_profile();
        let fixed = fix_with_rules(&original, &profile, &ApplyRules::default()).unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let title = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("JUDUL SKRIPSI"))
            .expect("judul sampul tidak ketemu");
        let run = title.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Arial"));
        assert_eq!(run.size(), Some(16.0));
        assert_eq!(title.alignment(), Some(Alignment::Center));
        let body = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Paragraf isi pertama"))
            .expect("paragraf isi tidak ketemu");
        let run = body.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Times New Roman"));
        assert_eq!(body.alignment(), Some(Alignment::Justify));
    }

    #[test]
    fn kalimat_gambar_tanpa_nomor_bukan_keterangan() {
        // "Gambar di bawah ini ..." adalah kalimat isi biasa: label
        // tidak langsung diikuti angka, jadi tetap diperbaiki.
        let original = fixture("uji.docx");
        let fixed = fix_docx(&original, "umum").unwrap();
        let doc = Document::from_bytes(&fixed).unwrap();
        let para = doc
            .paragraphs()
            .into_iter()
            .find(|p| p.text().contains("Gambar di bawah ini"))
            .expect("paragraf uji tidak ketemu");
        let run = para.runs().next().unwrap();
        assert_eq!(run.font_name(), Some("Times New Roman"));
        assert_eq!(run.size(), Some(12.0));
    }
}

