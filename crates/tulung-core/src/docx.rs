//! Doc Format: pindai dan perbaiki format .docx.
//!
//! Pindai membaca font, ukuran, spasi, margin, dan heading, lalu
//! melaporkan apa saja yang tidak sesuai preset. Perbaiki menerapkan
//! preset ke semua paragraf isi dan sel tabel, semua section, serta
//! font bawaan dokumen (docDefaults) supaya run tanpa format eksplisit
//! ikut berubah. Isi dokumen tidak disentuh: hanya format yang berubah.
//!
//! Aturan ukuran: run dengan ukuran 14 pt atau lebih dianggap judul
//! (judul sampul, subjudul) — font-nya diseragamkan, ukurannya
//! dipertahankan. Tanpa aturan ini, judul sampul makalah ikut mengecil
//! jadi 12 pt. Heading diperlakukan sama: hanya font yang diubah.
//!
//! Perbaikan juga membuat ulang daftar: daftar isi (dari heading),
//! daftar tabel/gambar yang sudah ada, dan daftar pustaka yang sudah
//! ada. Dokumen yang punya heading tapi belum ada daftar isi akan
//! dibuatkan daftar isinya di awal isi. Daftar pustaka dan daftar
//! tabel/gambar TIDAK dibuatkan bila belum ada — hanya yang sudah ada
//! yang dibuat ulang.
//!
//! Batasan yang diketahui: header/footer hanya dilaporkan (ada/tidak),
//! tidak ikut diperbaiki, karena rdocx 0.17 tidak membuka API tulis
//! untuk paragraf header/footer. Nomor halaman di daftar isi dihitung
//! layout internal rdocx dan bisa meleset dari pagination Word /
//! LibreOffice (terukur sampai 7 halaman pada dokumen 33 halaman):
//! judul dan struktur daftar benar, angka halaman adalah perkiraan.
//! Pengguna perlu Perbarui Field di Word/LibreOffice untuk angka eksak.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use rdocx::{Document, Length};
use serde::{Deserialize, Serialize};

/// Run dengan ukuran sebesar ini atau lebih dianggap judul: font-nya
/// diseragamkan, ukurannya dipertahankan (lihat catatan modul).
const TITLE_MIN_SIZE_PT: f64 = 14.0;

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

/// Baca preset bawaan dari nama yang dikirim shell web.
/// Satu preset universal.
pub fn preset(name: &str) -> Result<Profile, String> {
    match name {
        "umum" => Ok(builtin_profile()),
        other => Err(format!("Preset tidak dikenal: {other}")),
    }
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

/// Satu temuan dalam laporan pindai (hanya informasi: tidak ada
/// pemilihan di UI, perbaikan mengikuti profil yang dipilih).
#[derive(Debug, Serialize)]
pub struct Issue {
    /// Jenis: font, size, spacing, margin, heading, info.
    pub kind: &'static str,
    pub message: String,
    pub count: usize,
}

/// Laporan hasil pindai, dikirim ke shell web sebagai JSON.
#[derive(Serialize)]
pub struct ScanReport {
    pub paragraphs: usize,
    pub tables: usize,
    pub sections: usize,
    pub has_header: bool,
    pub has_footer: bool,
    pub fonts: Vec<Counted>,
    pub sizes: Vec<Counted>,
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

fn is_heading(style_id: Option<&str>) -> bool {
    style_id
        .map(|s| s.to_lowercase().contains("heading"))
        .unwrap_or(false)
}

/// Satu paragraf yang dipindai: gaya heading atau bukan, spasi,
/// dan daftar run (font, ukuran) di dalamnya.
struct ParaStat {
    heading: bool,
    spacing: Option<f64>,
    runs: Vec<(Option<String>, Option<f64>)>,
}

fn collect_paragraphs(doc: &Document) -> Vec<ParaStat> {
    let mut out = Vec::new();
    // Paragraf entri daftar dilewati: isinya dibuat ulang mesin,
    // bukan ditulis pengguna, jadi tidak dinilai terhadap profil.
    // Paragraf kosong dilewati juga (termasuk paragraf pembuka dan
    // penutup field daftar, yang memang tidak berisi kata).
    for p in doc.paragraphs() {
        if is_list_style(p.style_id()) || p.text().trim().is_empty() {
            continue;
        }
        out.push(para_stat(&p));
    }
    for table in doc.tables() {
        for row in 0..table.row_count() {
            for col in 0..table.column_count() {
                if let Some(cell) = table.cell(row, col) {
                    for p in cell.paragraphs() {
                        if is_list_style(p.style_id()) || p.text().trim().is_empty() {
                            continue;
                        }
                        out.push(para_stat(&p));
                    }
                }
            }
        }
    }
    out
}

fn para_stat(p: &rdocx::ParagraphRef) -> ParaStat {
    ParaStat {
        heading: is_heading(p.style_id()),
        spacing: p.line_spacing_multiple(),
        runs: p
            .runs()
            .map(|r| (r.font_name().map(str::to_string), r.size()))
            .collect(),
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
        bibliography_entries: None,
        headings,
    };
    let Ok(xml) = document_xml(bytes) else {
        return info;
    };
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

fn scan(doc: &Document, profile: &Profile, bytes: &[u8]) -> ScanReport {
    let paras = collect_paragraphs(doc);
    let mut fonts: BTreeMap<String, usize> = BTreeMap::new();
    let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
    let mut spacings: BTreeMap<String, usize> = BTreeMap::new();
    let mut issues: Vec<Issue> = Vec::new();

    for para in &paras {
        for (font, size) in &para.runs {
            let name = font.clone().unwrap_or_else(|| "Bawaan".to_string());
            *fonts.entry(name).or_insert(0) += 1;
            let size_text = size
                .map(|s| format!("{} pt", fmt_num(s)))
                .unwrap_or_else(|| "Bawaan".to_string());
            *sizes.entry(size_text).or_insert(0) += 1;
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
    let mut font_issue: BTreeMap<String, usize> = BTreeMap::new();
    let mut heading_font_issue: BTreeMap<String, usize> = BTreeMap::new();
    let mut size_issue: BTreeMap<String, usize> = BTreeMap::new();
    let mut title_kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut spacing_issue: BTreeMap<String, usize> = BTreeMap::new();
    for para in &paras {
        for (font, size) in &para.runs {
            if let Some(name) = font {
                if let Some(target) = &profile.font {
                    if name != target && !para.heading {
                        *font_issue.entry(name.clone()).or_insert(0) += 1;
                    }
                }
                if let Some(target) = &profile.heading_font {
                    if name != target && para.heading {
                        *heading_font_issue.entry(name.clone()).or_insert(0) += 1;
                    }
                }
            }
            if !para.heading {
                if let Some(size) = size {
                    if *size >= TITLE_MIN_SIZE_PT {
                        // Judul: ukurannya dipertahankan, bukan masalah.
                        *title_kept
                            .entry(format!("{} pt", fmt_num(*size)))
                            .or_insert(0) += 1;
                    } else if let Some(target) = profile.size_pt {
                        if (*size - target).abs() > 0.01 {
                            *size_issue
                                .entry(format!("{} pt", fmt_num(*size)))
                                .or_insert(0) += 1;
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
                        *spacing_issue
                            .entry(format!("Spasi {}", fmt_num(spacing)))
                            .or_insert(0) += 1;
                    }
                    None => {
                        *spacing_issue
                            .entry("Spasi tidak diatur".to_string())
                            .or_insert(0) += 1;
                    }
                }
            }
        }
    }
    for (name, count) in font_issue {
        issues.push(Issue {
            kind: "font",
            message: format!("Font {name} dipakai {count} kali"),
            count,
        });
    }
    for (name, count) in heading_font_issue {
        issues.push(Issue {
            kind: "heading",
            message: format!("Heading memakai font {name} ({count} kali)"),
            count,
        });
    }
    for (size_text, count) in size_issue {
        issues.push(Issue {
            kind: "size",
            message: format!("Ukuran {size_text} dipakai {count} kali"),
            count,
        });
    }
    for (size_text, count) in title_kept {
        issues.push(Issue {
            kind: "info",
            message: format!("Ukuran {size_text} dipertahankan sebagai judul ({count} kali)"),
            count,
        });
    }
    for (spacing_text, count) in spacing_issue {
        issues.push(Issue {
            kind: "spacing",
            message: format!("{spacing_text} dipakai {count} paragraf"),
            count,
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
                    message: format!("Font bawaan dokumen: {name}"),
                    count: 1,
                });
            }
        }
    }
    if let Some(size) = default_size_pt {
        if let Some(target) = profile.size_pt {
            if (size - target).abs() > 0.01 {
                issues.push(Issue {
                    kind: "size",
                    message: format!("Ukuran bawaan dokumen: {} pt", fmt_num(size)),
                    count: 1,
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
        let mut mismatched: Vec<String> = Vec::new();
        for (name, now, want) in sides {
            if let Some(want) = want {
                if (now - want).abs() >= 0.1 {
                    mismatched.push(format!(
                        "{name} {} cm (standar {} cm)",
                        fmt_num(now as f64),
                        fmt_num(want as f64)
                    ));
                }
            }
        }
        if !mismatched.is_empty() {
            issues.push(Issue {
                kind: "margin",
                message: format!("Margin saat ini: {}", mismatched.join(", ")),
                count: mismatched.len(),
            });
        }
    }

    let has_header = doc.header_text().map(|t| !t.trim().is_empty()).unwrap_or(false);
    let has_footer = doc.footer_text().map(|t| !t.trim().is_empty()).unwrap_or(false);
    if has_header || has_footer {
        issues.push(Issue {
            kind: "info",
            message: "Header/footer ada dan tidak ikut diubah".to_string(),
            count: 0,
        });
    }

    // Status daftar (hanya informasi, bukan masalah). Daftar pustaka
    // dan daftar tabel/gambar hanya dilaporkan bila sudah ada, karena
    // perbaikan tidak membuatkannya bila belum ada.
    let lists = list_info(doc, bytes);
    match lists.toc_entries {
        Some(n) => issues.push(Issue {
            kind: "info",
            message: format!("Daftar isi: {n} entri"),
            count: n,
        }),
        None if lists.headings > 0 => issues.push(Issue {
            kind: "info",
            message: "Tidak ada daftar isi, akan dibuatkan".to_string(),
            count: 0,
        }),
        None => issues.push(Issue {
            kind: "info",
            message: "Tidak ada daftar isi".to_string(),
            count: 0,
        }),
    }
    for table in &lists.tables {
        match table.entries {
            Some(n) => issues.push(Issue {
                kind: "info",
                message: format!("Daftar {}: {n} entri", table.label),
                count: n,
            }),
            None => issues.push(Issue {
                kind: "info",
                message: format!("Daftar {}: ada", table.label),
                count: 0,
            }),
        }
    }
    if let Some(n) = lists.bibliography_entries {
        issues.push(Issue {
            kind: "info",
            message: format!("Daftar pustaka: {n} entri"),
            count: n,
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
        sizes: to_counted(sizes),
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

fn fix_with_profile(bytes: &[u8], profile: &Profile) -> Result<Vec<u8>, String> {
    let mut doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    apply_fix(&mut doc, profile)?;
    let headings = doc
        .paragraphs()
        .iter()
        .filter(|p| is_heading(p.style_id()))
        .count();
    let mut fixed = doc.to_bytes().map_err(|err| err.to_string())?;
    // Dokumen punya heading tapi belum ada daftar isi: buatkan
    // skeleton-nya di awal isi, supaya rebuild_toc bisa mengisinya.
    // (rdocx tidak bisa menyisip field kompleks multi-paragraf lewat
    // API; skeleton disisip lewat bedah zip sebelum dokumen dimuat.)
    let has_toc = document_xml(&fixed)
        .map(|xml| field_spans(&xml).iter().any(|s| matches!(s.kind, FieldKind::Toc)))
        .unwrap_or(false);
    // Lepas dokumen pertama sebelum memuat ulang: dua Document rdocx
    // hidup bersamaan pernah menghabiskan stack thread tes.
    drop(doc);
    if headings > 0 && !has_toc {
        fixed = insert_toc_skeleton(&fixed)?;
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

/// Sisip skeleton daftar isi tepat setelah tag <w:body> (awal isi).
/// Semua entri zip disalin; hanya word/document.xml yang diubah.
fn insert_toc_skeleton(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut xml = document_xml(bytes)?;
    let body = xml.find("<w:body").ok_or("Tag <w:body> tidak ketemu")?;
    let close = xml[body..]
        .find('>')
        .ok_or("Tag <w:body> tidak tertutup")?
        + body;
    xml.insert_str(close + 1, TOC_SKELETON);
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

fn apply_fix(doc: &mut Document, profile: &Profile) -> Result<(), String> {
    // Paragraf isi. Penanda heading dan ukuran run dibaca dulu dari
    // sisi baca, karena Paragraph/Run (tulis) tidak punya getter
    // style_id/size.
    let para_info: Vec<(bool, Vec<Option<f64>>)> = doc
        .paragraphs()
        .iter()
        .map(|p| {
            (
                is_heading(p.style_id()),
                p.runs().map(|r| r.size()).collect(),
            )
        })
        .collect();
    for (i, (heading, run_sizes)) in para_info.iter().enumerate() {
        if let Some(mut p) = doc.paragraph_mut(i) {
            fix_paragraph(&mut p, profile, *heading, run_sizes);
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
        let mut flags: Vec<Vec<Vec<(bool, Vec<Option<f64>>)>>> = Vec::new();
        if let Some(table_ref) = doc.tables().get(ti) {
            for row in 0..*rows {
                let mut row_flags = Vec::new();
                for col in 0..*cols {
                    let cell_flags = table_ref
                        .cell(row, col)
                        .map(|cell| {
                            cell.paragraphs()
                                .map(|p| {
                                    (
                                        is_heading(p.style_id()),
                                        p.runs().map(|r| r.size()).collect(),
                                    )
                                })
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
                                let (heading, run_sizes) = flags
                                    .get(row)
                                    .and_then(|r| r.get(col))
                                    .and_then(|c| c.get(pi))
                                    .cloned()
                                    .unwrap_or((false, Vec::new()));
                                fix_paragraph(&mut p, profile, heading, &run_sizes);
                            }
                        }
                    }
                }
            }
        }
    }
    // Semua section. Hanya sisi margin yang diatur profil yang
    // diubah; sisi lain dipertahankan.
    let target = profile.margins();
    if target.0.is_some() || target.1.is_some() || target.2.is_some() || target.3.is_some() {
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
    if let Some(font) = &profile.font {
        doc.set_default_font_name_value(Some(font))
            .map_err(|err| err.to_string())?;
    }
    if let Some(size) = profile.size_pt {
        doc.set_default_font_size_value(Some(Length::pt(size)))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Perbaiki satu paragraf sesuai profil: heading hanya diubah font-nya
/// (ukuran dan spasi dipertahankan); paragraf isi diubah font, ukuran,
/// dan spasi, kecuali run berukuran judul (>= 14 pt) yang ukurannya
/// dipertahankan. Aspek yang tidak diatur profil (None) tidak disentuh
/// sama sekali.
fn fix_paragraph(
    p: &mut rdocx::Paragraph,
    profile: &Profile,
    heading: bool,
    run_sizes: &[Option<f64>],
) {
    if !heading {
        if let Some(target) = profile.line_spacing {
            p.set_line_spacing_multiple(target);
        }
    }
    let font_target: Option<&str> = if heading {
        profile.heading_font.as_deref()
    } else {
        profile.font.as_deref()
    };
    let run_count = p.run_count();
    for j in 0..run_count {
        if let Some(mut run) = p.run_mut(j) {
            if let Some(target) = font_target {
                run.set_font(target);
            }
            if !heading {
                if let Some(target) = profile.size_pt {
                    let is_title = run_sizes
                        .get(j)
                        .copied()
                        .flatten()
                        .map(|size| size >= TITLE_MIN_SIZE_PT)
                        .unwrap_or(false);
                    if !is_title {
                        run.set_size(target);
                    }
                }
            }
        }
    }
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
/// perbaikan tidak mengubah isi. Paragraf entri daftar dan paragraf
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
        let actionable: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.kind != "info")
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
        assert!(
            after.issues.iter().all(|i| i.kind == "info"),
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
    fn isi_tidak_berubah_setelah_diperbaiki() {
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
        // Pindai ulang: ukuran judul hanya jadi catatan, bukan masalah.
        let report = scan_json(&fixed, "umum");
        let actionable: Vec<_> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"].as_str().unwrap() != "info")
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
        // Isi identik: entri daftar (basi maupun baru) bukan isi.
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
        // Isi identik (entri daftar baru tidak dihitung sebagai isi).
        assert_eq!(
            all_text(&Document::from_bytes(&original).unwrap()),
            all_text(&Document::from_bytes(&fixed).unwrap())
        );
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
}

