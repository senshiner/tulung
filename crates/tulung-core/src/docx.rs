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
//! Batasan yang diketahui: header/footer hanya dilaporkan (ada/tidak),
//! tidak ikut diperbaiki, karena rdocx 0.17 tidak membuka API tulis
//! untuk paragraf header/footer.

use std::collections::{BTreeMap, HashSet};

use rdocx::{Document, Length};
use serde::Serialize;

/// Run dengan ukuran sebesar ini atau lebih dianggap judul: font-nya
/// diseragamkan, ukurannya dipertahankan (lihat catatan modul).
const TITLE_MIN_SIZE_PT: f64 = 14.0;

/// Preset format.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub font: &'static str,
    pub size_pt: f64,
    pub line_spacing: f64,
    /// Margin (atas, kanan, bawah, kiri) dalam cm.
    pub margins_cm: (f32, f32, f32, f32),
}

/// Preset "Standar Umum": Times New Roman 12, spasi 1,5,
/// margin atas 4, kanan 3, bawah 3, kiri 3 cm.
pub const PRESET_UMUM: Preset = Preset {
    font: "Times New Roman",
    size_pt: 12.0,
    line_spacing: 1.5,
    margins_cm: (4.0, 3.0, 3.0, 3.0),
};

/// Baca preset dari nama yang dikirim shell web. Satu preset universal.
pub fn preset(name: &str) -> Result<Preset, String> {
    match name {
        "umum" => Ok(PRESET_UMUM),
        other => Err(format!("Preset tidak dikenal: {other}")),
    }
}

/// Satu baris hitungan untuk laporan: nilai tampil + jumlah kemunculan.
#[derive(Serialize)]
pub struct Counted {
    pub value: String,
    pub count: usize,
}

/// Satu temuan dalam laporan. `id` dipakai shell web untuk memilih
/// temuan mana yang diperbaiki (misal `font:Calibri`, `size:11`,
/// `spacing:1`, `spacing:tidak-diatur`, `margin`, `default-font`).
#[derive(Serialize)]
pub struct Issue {
    /// Jenis: font, size, spacing, margin, heading, info.
    pub kind: &'static str,
    /// Id stabil untuk pemilihan (kosong hanya untuk catatan info).
    pub id: String,
    pub message: String,
    pub count: usize,
}

/// Format angka untuk id: titik sebagai pemisah desimal, tanpa nol
/// di belakang koma (11 -> "11", 11.5 -> "11.5").
fn id_num(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
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
}

/// Format angka: buang ,0 di belakang, pakai koma desimal.
fn fmt_num(value: f64) -> String {
    if (value - value.round()).abs() < 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}").replace('.', ",")
    }
}

fn fmt_margins(margins: (f32, f32, f32, f32)) -> String {
    format!(
        "{}-{}-{}-{}",
        fmt_num(margins.0 as f64),
        fmt_num(margins.1 as f64),
        fmt_num(margins.2 as f64),
        fmt_num(margins.3 as f64),
    )
}

fn margins_close(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    // Toleransi 1 mm (pembulatan twips).
    (a.0 - b.0).abs() < 0.1
        && (a.1 - b.1).abs() < 0.1
        && (a.2 - b.2).abs() < 0.1
        && (a.3 - b.3).abs() < 0.1
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
    for p in doc.paragraphs() {
        out.push(para_stat(&p));
    }
    for table in doc.tables() {
        for row in 0..table.row_count() {
            for col in 0..table.column_count() {
                if let Some(cell) = table.cell(row, col) {
                    for p in cell.paragraphs() {
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
    let preset = preset(preset_name)?;
    let doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    let report = scan(&doc, &preset);
    serde_json::to_string(&report).map_err(|err| err.to_string())
}

fn scan(doc: &Document, preset: &Preset) -> ScanReport {
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
    let mut font_issue: BTreeMap<String, usize> = BTreeMap::new();
    let mut heading_font_issue: BTreeMap<String, usize> = BTreeMap::new();
    let mut size_issue: BTreeMap<String, usize> = BTreeMap::new();
    let mut title_kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut spacing_issue: BTreeMap<String, usize> = BTreeMap::new();
    for para in &paras {
        for (font, size) in &para.runs {
            if let Some(name) = font {
                if name != preset.font {
                    if para.heading {
                        *heading_font_issue.entry(name.clone()).or_insert(0) += 1;
                    } else {
                        *font_issue.entry(name.clone()).or_insert(0) += 1;
                    }
                }
            }
            if !para.heading {
                if let Some(size) = size {
                    if *size >= TITLE_MIN_SIZE_PT {
                        // Judul: ukurannya dipertahankan, bukan masalah.
                        *title_kept
                            .entry(id_num(*size))
                            .or_insert(0) += 1;
                    } else if (*size - preset.size_pt).abs() > 0.01 {
                        *size_issue.entry(id_num(*size)).or_insert(0) += 1;
                    }
                }
            }
        }
        if !para.heading {
            match para.spacing {
                Some(spacing) if (spacing - preset.line_spacing).abs() < 0.01 => {}
                Some(spacing) => {
                    *spacing_issue.entry(id_num(spacing)).or_insert(0) += 1;
                }
                None => {
                    *spacing_issue
                        .entry("tidak-diatur".to_string())
                        .or_insert(0) += 1;
                }
            }
        }
    }
    for (name, count) in font_issue {
        issues.push(Issue {
            kind: "font",
            id: format!("font:{name}"),
            message: format!("Font {name} dipakai {count} kali"),
            count,
        });
    }
    for (name, count) in heading_font_issue {
        issues.push(Issue {
            kind: "heading",
            id: format!("heading:{name}"),
            message: format!("Heading memakai font {name} ({count} kali)"),
            count,
        });
    }
    for (size_id, count) in size_issue {
        issues.push(Issue {
            kind: "size",
            id: format!("size:{size_id}"),
            message: format!(
                "Ukuran {} pt dipakai {count} kali",
                size_id.replace('.', ",")
            ),
            count,
        });
    }
    for (size_id, count) in title_kept {
        issues.push(Issue {
            kind: "info",
            id: format!("info:judul-{size_id}"),
            message: format!(
                "Ukuran {} pt dipertahankan sebagai judul ({count} kali)",
                size_id.replace('.', ",")
            ),
            count,
        });
    }
    for (spacing_id, count) in spacing_issue {
        let message = if spacing_id == "tidak-diatur" {
            format!("Spasi tidak diatur dipakai {count} paragraf")
        } else {
            format!(
                "Spasi {} dipakai {count} paragraf",
                spacing_id.replace('.', ",")
            )
        };
        issues.push(Issue {
            kind: "spacing",
            id: format!("spacing:{spacing_id}"),
            message,
            count,
        });
    }

    // Font bawaan dokumen (docDefaults).
    let default_font = doc.default_font_name().map(str::to_string);
    let default_size_pt = doc.default_font_size().map(|len| len.to_pt());
    if let Some(name) = &default_font {
        if name != preset.font {
            issues.push(Issue {
                kind: "font",
                id: "default-font".to_string(),
                message: format!("Font bawaan dokumen: {name}"),
                count: 1,
            });
        }
    }
    if let Some(size) = default_size_pt {
        if (size - preset.size_pt).abs() > 0.01 {
            issues.push(Issue {
                kind: "size",
                id: "default-size".to_string(),
                message: format!("Ukuran bawaan dokumen: {} pt", fmt_num(size)),
                count: 1,
            });
        }
    }

    // Margin section pertama.
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
        if !margins_close(current, preset.margins_cm) {
            issues.push(Issue {
                kind: "margin",
                id: "margin".to_string(),
                message: format!("Margin saat ini {} cm", fmt_margins(current)),
                count: 1,
            });
        }
    }

    let has_header = doc.header_text().map(|t| !t.trim().is_empty()).unwrap_or(false);
    let has_footer = doc.footer_text().map(|t| !t.trim().is_empty()).unwrap_or(false);
    if has_header || has_footer {
        issues.push(Issue {
            kind: "info",
            id: "info:header-footer".to_string(),
            message: "Header/footer ada dan tidak ikut diubah".to_string(),
            count: 0,
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
    }
}

/// Pilihan perbaikan: id temuan yang dicentang pengguna di rapor pindai.
/// Daftar kosong berarti perbaiki semua temuan yang bisa diperbaiki.
pub struct Selection {
    fonts: HashSet<String>,
    heading_fonts: HashSet<String>,
    sizes: HashSet<String>,
    spacings: HashSet<String>,
    margin: bool,
    default_font: bool,
    default_size: bool,
    all: bool,
}

impl Selection {
    pub fn all() -> Self {
        Self {
            fonts: HashSet::new(),
            heading_fonts: HashSet::new(),
            sizes: HashSet::new(),
            spacings: HashSet::new(),
            margin: false,
            default_font: false,
            default_size: false,
            all: true,
        }
    }

    pub fn from_ids(ids: &[String]) -> Self {
        if ids.is_empty() {
            return Self::all();
        }
        let mut sel = Self {
            fonts: HashSet::new(),
            heading_fonts: HashSet::new(),
            sizes: HashSet::new(),
            spacings: HashSet::new(),
            margin: false,
            default_font: false,
            default_size: false,
            all: false,
        };
        for id in ids {
            if let Some(name) = id.strip_prefix("font:") {
                sel.fonts.insert(name.to_string());
            } else if let Some(name) = id.strip_prefix("heading:") {
                sel.heading_fonts.insert(name.to_string());
            } else if let Some(value) = id.strip_prefix("size:") {
                sel.sizes.insert(value.to_string());
            } else if let Some(value) = id.strip_prefix("spacing:") {
                sel.spacings.insert(value.to_string());
            } else if id == "margin" {
                sel.margin = true;
            } else if id == "default-font" {
                sel.default_font = true;
            } else if id == "default-size" {
                sel.default_size = true;
            }
            // Id lain (info:...) diabaikan: bukan temuan yang bisa diperbaiki.
        }
        sel
    }

    fn wants_font(&self, name: &str) -> bool {
        self.all || self.fonts.contains(name)
    }

    fn wants_heading_font(&self, name: &str) -> bool {
        self.all || self.heading_fonts.contains(name)
    }

    fn wants_size(&self, size: f64) -> bool {
        self.all || self.sizes.contains(&id_num(size))
    }

    fn wants_spacing(&self, spacing: Option<f64>) -> bool {
        if self.all {
            return true;
        }
        match spacing {
            Some(value) => self.spacings.contains(&id_num(value)),
            None => self.spacings.contains("tidak-diatur"),
        }
    }
}

/// Perbaiki .docx sesuai preset, kembalikan byte .docx baru.
/// `selected` berisi id temuan yang dicentang pengguna; kosong berarti
/// perbaiki semua temuan yang bisa diperbaiki.
pub fn fix_docx(
    bytes: &[u8],
    preset_name: &str,
    selected: &[String],
) -> Result<Vec<u8>, String> {
    let preset = preset(preset_name)?;
    let mut doc = Document::from_bytes(bytes)
        .map_err(|err| format!("File bukan .docx yang valid: {err}"))?;
    let selection = Selection::from_ids(selected);
    apply_fix(&mut doc, &preset, &selection)?;
    doc.to_bytes().map_err(|err| err.to_string())
}

fn apply_fix(doc: &mut Document, preset: &Preset, selection: &Selection) -> Result<(), String> {
    // Paragraf isi. Penanda heading, ukuran run, font run, dan spasi
    // dibaca dulu dari sisi baca, karena Paragraph/Run (tulis) tidak
    // punya getter style_id/size/font.
    let para_info: Vec<(bool, Vec<Option<f64>>, Vec<Option<String>>, Option<f64>)> = doc
        .paragraphs()
        .iter()
        .map(|p| {
            (
                is_heading(p.style_id()),
                p.runs().map(|r| r.size()).collect(),
                p.runs().map(|r| r.font_name().map(str::to_string)).collect(),
                p.line_spacing_multiple(),
            )
        })
        .collect();
    for (i, (heading, run_sizes, run_fonts, spacing)) in para_info.iter().enumerate() {
        if let Some(mut p) = doc.paragraph_mut(i) {
            fix_paragraph(&mut p, preset, selection, *heading, run_sizes, run_fonts, *spacing);
        }
    }
    // Paragraf di sel tabel.
    let tables: Vec<(usize, usize)> = doc
        .tables()
        .iter()
        .map(|t| (t.row_count(), t.column_count()))
        .collect();
    for (ti, (rows, cols)) in tables.iter().enumerate() {
        // Penanda heading, ukuran run, font run, dan spasi per sel,
        // dibaca dulu dari sisi baca.
        let mut flags: Vec<Vec<Vec<(bool, Vec<Option<f64>>, Vec<Option<String>>, Option<f64>)>>> =
            Vec::new();
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
                                        p.runs()
                                            .map(|r| r.font_name().map(str::to_string))
                                            .collect(),
                                        p.line_spacing_multiple(),
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
                                let (heading, run_sizes, run_fonts, spacing) = flags
                                    .get(row)
                                    .and_then(|r| r.get(col))
                                    .and_then(|c| c.get(pi))
                                    .cloned()
                                    .unwrap_or((false, Vec::new(), Vec::new(), None));
                                fix_paragraph(
                                    &mut p, preset, selection, heading, &run_sizes, &run_fonts,
                                    spacing,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    // Semua section, hanya bila margin dicentang.
    if selection.all || selection.margin {
        let section_count = doc.sections().count();
        for i in 0..section_count {
            if let Some(mut section) = doc.section_mut(i) {
                section
                    .set_margins(
                        Length::cm(preset.margins_cm.0 as f64),
                        Length::cm(preset.margins_cm.1 as f64),
                        Length::cm(preset.margins_cm.2 as f64),
                        Length::cm(preset.margins_cm.3 as f64),
                    )
                    .map_err(|err| err.to_string())?;
            }
        }
    }
    // Font bawaan dokumen, supaya run tanpa format eksplisit ikut berubah.
    if selection.all || selection.default_font {
        doc.set_default_font_name_value(Some(preset.font))
            .map_err(|err| err.to_string())?;
    }
    if selection.all || selection.default_size {
        doc.set_default_font_size_value(Some(Length::pt(preset.size_pt)))
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Perbaiki satu paragraf sesuai pilihan: heading hanya diubah font-nya
/// (ukuran dan spasi dipertahankan); paragraf isi diubah font, ukuran,
/// dan spasi, kecuali run berukuran judul (>= 14 pt) yang ukurannya
/// dipertahankan. Hanya temuan yang dicentang yang diperbaiki.
fn fix_paragraph(
    p: &mut rdocx::Paragraph,
    preset: &Preset,
    selection: &Selection,
    heading: bool,
    run_sizes: &[Option<f64>],
    run_fonts: &[Option<String>],
    spacing: Option<f64>,
) {
    if !heading && selection.wants_spacing(spacing) {
        p.set_line_spacing_multiple(preset.line_spacing);
    }
    let run_count = p.run_count();
    for j in 0..run_count {
        if let Some(mut run) = p.run_mut(j) {
            let font = run_fonts.get(j).cloned().flatten();
            // Run tanpa font eksplisit ikut diubah hanya dalam mode
            // "semua" (perilaku lama); dalam mode selektif ia mengikuti
            // font bawaan dokumen, yang hanya berubah bila
            // "default-font" dicentang.
            let set_font = match &font {
                Some(name) if heading => selection.wants_heading_font(name),
                Some(name) => selection.wants_font(name),
                None => selection.all,
            };
            if set_font {
                run.set_font(preset.font);
            }
            if !heading {
                let set_size = match run_sizes.get(j).copied().flatten() {
                    Some(size) => size < TITLE_MIN_SIZE_PT && selection.wants_size(size),
                    None => selection.all,
                };
                if set_size {
                    run.set_size(preset.size_pt);
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

/// Seluruh teks dokumen (paragraf isi + sel tabel), untuk memastikan
/// perbaikan tidak mengubah isi.
pub fn all_text(doc: &Document) -> String {
    let mut parts = Vec::new();
    for p in doc.paragraphs() {
        parts.push(p.text());
    }
    for table in doc.tables() {
        for row in 0..table.row_count() {
            for col in 0..table.column_count() {
                if let Some(cell) = table.cell(row, col) {
                    for p in cell.paragraphs() {
                        parts.push(p.text());
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
        // 4 paragraf isi (1 heading + 3) + 4 paragraf sel tabel (2x2).
        assert_eq!(report["paragraphs"], 8);
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
    fn perbaiki_selektif_hanya_font() {
        let original = fixture("campuran.docx");
        let report = scan_json(&original, "umum");
        let font_ids: Vec<String> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"].as_str().unwrap() == "font")
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect();
        assert!(!font_ids.is_empty(), "campuran harus punya temuan font");
        assert!(font_ids.contains(&"font:Calibri".to_string()));
        // Template bawaan python-docx memakai font tema, jadi tidak ada
        // temuan "default-font" pada fixture ini.
        let fixed = fix_docx(&original, "umum", &font_ids).unwrap();
        let after = scan_json(&fixed, "umum");
        let kinds = issue_kinds(&after);
        assert!(
            !kinds.contains(&"font".to_string()),
            "temuan font harus hilang: {}",
            after["issues"]
        );
        // Temuan lain yang tidak dicentang harus tetap ada.
        assert!(kinds.contains(&"size".to_string()), "kinds: {kinds:?}");
        assert!(kinds.contains(&"spacing".to_string()));
        assert!(kinds.contains(&"margin".to_string()));
    }

    #[test]
    fn perbaiki_selektif_satu_ukuran() {
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum", &["size:11".to_string()]).unwrap();
        let after = scan_json(&fixed, "umum");
        let ids: Vec<String> = after["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect();
        assert!(!ids.contains(&"size:11".to_string()), "ids: {ids:?}");
        // Ukuran bawaan (default-size) tidak dicentang, jadi tetap ada.
        assert!(ids.contains(&"default-size".to_string()), "ids: {ids:?}");
        // Font tidak dicentang, tetap ada.
        assert!(ids.iter().any(|id| id.starts_with("font:")), "ids: {ids:?}");
    }

    #[test]
    fn id_masalah_stabil_untuk_pemilihan() {
        let report = scan_json(&fixture("campuran.docx"), "umum");
        let ids: Vec<String> = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect();
        assert!(ids.contains(&"font:Calibri".to_string()), "ids: {ids:?}");
        assert!(ids.contains(&"size:11".to_string()), "ids: {ids:?}");
        assert!(ids.contains(&"spacing:1".to_string()), "ids: {ids:?}");
        assert!(ids.contains(&"spacing:2".to_string()), "ids: {ids:?}");
        assert!(ids.contains(&"spacing:tidak-diatur".to_string()), "ids: {ids:?}");
        assert!(ids.contains(&"margin".to_string()), "ids: {ids:?}");
    }

    #[test]
    fn perbaiki_lalu_pindai_ulang_bersih() {
        let original = fixture("campuran.docx");
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        let fixed = fix_docx(&original, "umum", &[]).unwrap();
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
        assert!(fix_docx(b"bukan docx sama sekali", "umum", &[]).is_err());
        assert!(scan_docx(&[], "umum").is_err());
    }

    #[test]
    fn preset_tidak_dikenal_ditolak() {
        let original = fixture("campuran.docx");
        assert!(scan_docx(&original, "kampus-lain").is_err());
        assert!(fix_docx(&original, "kampus-lain", &[]).is_err());
    }
}
