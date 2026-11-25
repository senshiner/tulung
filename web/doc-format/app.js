// Doc Format: shell web untuk tulung-docx (WASM).
// Alur: baca .docx sebagai byte -> pindai dengan scan_docx_profile() di WASM
// -> tampilkan daftar masalah -> perbaiki dengan fix_docx_profile() -> unduh
// + pindai ulang hasil perbaikan. Tidak ada server; teks isi
// tidak berubah, hanya formatnya. Daftar isi, tabel, dan gambar
// dibuat atau dibangun ulang bila perlu.
//
// WASM dimuat dinamis di dalam handler, bukan import statis di atas:
// kalau modul WASM gagal dimuat, pemilih dokumen tetap jalan dan
// pengguna dapat pesan error yang jelas, bukan halaman yang diam saja.

const dropArea = document.getElementById('drop-area');
const fileInput = document.getElementById('file-input');
const fileSection = document.getElementById('file-section');
const fileNameEl = document.getElementById('file-name');
const fileSizeEl = document.getElementById('file-size');
const removeFileBtn = document.getElementById('remove-file');
const profileSection = document.getElementById('profile-section');
const presetSelect = document.getElementById('preset-select');
const profileNameInput = document.getElementById('profile-name');
const profileFontInput = document.getElementById('profile-font');
const profileSizeInput = document.getElementById('profile-size');
const profileSpacingInput = document.getElementById('profile-spacing');
const profileMarginTopInput = document.getElementById('profile-margin-top');
const profileMarginRightInput = document.getElementById('profile-margin-right');
const profileMarginBottomInput = document.getElementById('profile-margin-bottom');
const profileMarginLeftInput = document.getElementById('profile-margin-left');
const profileHeadingFontInput = document.getElementById('profile-heading-font');
const shareProfileBtn = document.getElementById('share-profile-btn');
const profileStatus = document.getElementById('profile-status');
const scanSection = document.getElementById('scan-section');
const scanSummary = document.getElementById('scan-summary');
const issueList = document.getElementById('issue-list');
const rulesList = document.getElementById('rules-list');
const pageNumberingSection = document.getElementById('page-numbering-section');
const coverEndSelect = document.getElementById('cover-end-select');
const contentStartSelect = document.getElementById('content-start-select');
const fixBtn = document.getElementById('fix-btn');
const loadingEl = document.getElementById('loading');
const errorEl = document.getElementById('error');
const resultSection = document.getElementById('result-section');
const filenameInput = document.getElementById('filename');
const downloadLink = document.getElementById('download-link');
const rescanResult = document.getElementById('rescan-result');
const pageNumberWarning = document.getElementById('page-number-warning');
const remainingList = document.getElementById('remaining-list');
const compareSection = document.getElementById('compare-section');
const compareBody = document.getElementById('compare-body');
const textVerdict = document.getElementById('text-verdict');

const DOCX_TYPE = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';

// Kelompok aturan di daftar centang "Aturan yang diterapkan", sesuai
// kelompok temuan (group) dari laporan pindai WASM. Penomoran halaman
// tidak punya temuan pindai: ia opt-in (tidak dicentang di awal) dan
// butuh dua batas yang dipilih pengguna (lihat pemilih batas).
const RULE_GROUPS = [
  { key: 'margins', label: 'Margin halaman' },
  { key: 'font', label: 'Jenis font' },
  { key: 'size', label: 'Ukuran teks' },
  { key: 'spacing', label: 'Spasi baris' },
  { key: 'indent', label: 'Indentasi' },
  { key: 'alignment', label: 'Perataan' },
  { key: 'headings', label: 'Heading BAB & subbab' },
  { key: 'paragraph_spacing', label: 'Jarak antarparagraf' },
  { key: 'page_numbering', label: 'Penomoran halaman' },
  { key: 'lists', label: 'Daftar isi' },
];

// Pilihan aturan pengguna; semua dicentang secara default dan tetap
// dipertahankan antar pindai ulang. Penomoran halaman pengecualian:
// tidak dicentang di awal karena butuh batas yang dipilih pengguna.
const ruleSelection = {};
RULE_GROUPS.forEach(({ key }) => { ruleSelection[key] = true; });
ruleSelection.page_numbering = false;

// Batas penomoran halaman: indeks paragraf (urutan doc.paragraphs)
// terakhir sampul dan pertama isi. null = belum dipilih.
let pageBounds = { coverEnd: null, contentStart: null };
let wasmSupportsPageNumbering = false;

let docFile = null; // { name, data: Uint8Array }
let wasmPromise = null;
let docUrl = null;
let lastReport = null; // Laporan pindai sebelum perbaikan (untuk banding).
let rescanTimer = null;
let fixing = false; // Perbaikan sedang jalan: pindai otomatis harus diam.

function loadWasm() {
  if (!wasmPromise) {
    wasmPromise = import('../pkg-docx/tulung_docx.js').then(async (mod) => {
      await mod.default();
      wasmSupportsPageNumbering = typeof mod.docx_paragraphs === 'function';
      return mod;
    });
  }
  return wasmPromise;
}

function formatNum(value) {
  return value === null || value === undefined
    ? ''
    : String(value).replace('.', ',');
}

function parseNum(value) {
  const text = value.trim().replace(',', '.');
  if (!text) return null;
  const num = Number(text);
  return Number.isFinite(num) ? num : null;
}

function setFontSelect(select, value) {
  // Font dari link profil bisa di luar daftar: tambahkan sebagai opsi
  // supaya nilainya tetap terbaca dan bisa dipilih lagi.
  if (value && !Array.from(select.options).some((opt) => opt.value === value)) {
    const extra = document.createElement('option');
    extra.value = value;
    extra.textContent = value;
    select.appendChild(extra);
  }
  select.value = value || '';
}

function fillProfileForm(profile) {
  profileNameInput.value = profile.name || '';
  setFontSelect(profileFontInput, profile.font);
  profileSizeInput.value = formatNum(profile.size_pt);
  profileSpacingInput.value = formatNum(profile.line_spacing);
  profileMarginTopInput.value = formatNum(profile.margin_top_cm);
  profileMarginRightInput.value = formatNum(profile.margin_right_cm);
  profileMarginBottomInput.value = formatNum(profile.margin_bottom_cm);
  profileMarginLeftInput.value = formatNum(profile.margin_left_cm);
  setFontSelect(profileHeadingFontInput, profile.heading_font);
}

function profileFromForm() {
  const text = (input) => {
    const value = input.value.trim();
    return value || null;
  };
  return {
    name: profileNameInput.value.trim() || 'Profil Kustom',
    font: text(profileFontInput),
    size_pt: parseNum(profileSizeInput.value),
    line_spacing: parseNum(profileSpacingInput.value),
    margin_top_cm: parseNum(profileMarginTopInput.value),
    margin_right_cm: parseNum(profileMarginRightInput.value),
    margin_bottom_cm: parseNum(profileMarginBottomInput.value),
    margin_left_cm: parseNum(profileMarginLeftInput.value),
    heading_font: text(profileHeadingFontInput),
  };
}

function toBase64Url(obj) {
  const bytes = new TextEncoder().encode(JSON.stringify(obj));
  let binary = '';
  bytes.forEach((b) => { binary += String.fromCharCode(b); });
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

function fromBase64Url(str) {
  const base64 = str.replaceAll('-', '+').replaceAll('_', '/');
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
  return JSON.parse(new TextDecoder().decode(bytes));
}

function profileFromHash() {
  const match = window.location.hash.match(/#profil=([A-Za-z0-9_-]+)/);
  if (!match) return null;
  try {
    return fromBase64Url(match[1]);
  } catch {
    return null;
  }
}

async function initProfileForm() {
  try {
    const wasm = await loadWasm();
    const shared = profileFromHash();
    if (shared) {
      fillProfileForm(shared);
      presetSelect.value = 'kustom';
      profileStatus.textContent = `Profil dari link: ${shared.name}.`;
    } else if (typeof wasm.preset_profile === 'function') {
      fillProfileForm(JSON.parse(wasm.preset_profile('umum')));
      presetSelect.value = 'umum';
    } else {
      fillProfileForm(JSON.parse(wasm.default_profile()));
    }
  } catch {
    // Formulir tetap bisa dipakai tanpa WASM; angka default ditulis
    // ulang saat pemindaian pertama berhasil.
  }
}

// Memilih preset mengisi formulir dengan nilai preset itu. Mengubah
// kolom sendiri sesudahnya mengubah pilihan jadi "Profil kustom"
// (lihat pendengar input di bawah).
presetSelect.addEventListener('change', async () => {
  const name = presetSelect.value;
  if (name === 'kustom') return;
  try {
    const wasm = await loadWasm();
    if (typeof wasm.preset_profile !== 'function') return;
    fillProfileForm(JSON.parse(wasm.preset_profile(name)));
    if (docFile) scanDocument();
  } catch {
    showError('Gagal memuat preset. Coba muat ulang halaman.');
  }
});

// Format yang dipakai pindai dan perbaikan selalu dibaca dari kolom
// formulir saat itu juga.
async function scanActive(bytes) {
  const wasm = await loadWasm();
  return JSON.parse(wasm.scan_docx_profile(bytes, JSON.stringify(profileFromForm())));
}

async function fixActive(bytes) {
  const wasm = await loadWasm();
  const profileJson = JSON.stringify(profileFromForm());
  // Mesin baru menerima pilihan aturan; mesin lama (WASM belum
  // diperbarui) jatuh ke perbaikan penuh seperti sebelumnya.
  if (typeof wasm.fix_docx_profile_rules === 'function') {
    const rules = { ...ruleSelection };
    if (rules.page_numbering && boundsValid()) {
      rules.page_bounds = {
        cover_end: pageBounds.coverEnd,
        content_start: pageBounds.contentStart,
      };
    }
    return wasm.fix_docx_profile_rules(bytes, profileJson, JSON.stringify(rules));
  }
  return wasm.fix_docx_profile(bytes, profileJson);
}

// Batas penomoran valid: keduanya dipilih dan akhir sampul
// benar-benar sebelum awal isi.
function boundsValid() {
  return pageBounds.coverEnd != null && pageBounds.contentStart != null
    && pageBounds.coverEnd < pageBounds.contentStart;
}

// Isi kedua pemilih batas dengan daftar paragraf dokumen.
// Pilihan lama dipertahankan bila indeksnya masih ada.
function fillBoundarySelects(paragraphs) {
  [coverEndSelect, contentStartSelect].forEach((select) => {
    const prev = select.value;
    select.innerHTML = '';
    const placeholder = document.createElement('option');
    placeholder.value = '';
    placeholder.textContent = '— pilih paragraf —';
    select.appendChild(placeholder);
    paragraphs.forEach(({ index, text }) => {
      const opt = document.createElement('option');
      opt.value = String(index);
      const snippet = (text.trim() || '(paragraf kosong)').slice(0, 60);
      opt.textContent = `${index + 1}. ${snippet}`;
      select.appendChild(opt);
    });
    if (prev !== '' && Number(prev) < paragraphs.length) select.value = prev;
  });
}

// Muat daftar paragraf untuk pemilih batas (butuh WASM baru).
// Gagal dimuat bukan fatal: penomoran akan ditolak dengan pesan
// yang jelas bila dicentang tanpa batas.
async function loadPageParagraphs() {
  if (!docFile || !wasmSupportsPageNumbering) return;
  try {
    const wasm = await loadWasm();
    if (typeof wasm.docx_paragraphs !== 'function') return;
    fillBoundarySelects(JSON.parse(wasm.docx_paragraphs(docFile.data)));
  } catch {
    // Pemilih batas tetap kosong.
  }
}

function onBoundaryChange() {
  pageBounds = {
    coverEnd: coverEndSelect.value === '' ? null : Number(coverEndSelect.value),
    contentStart: contentStartSelect.value === '' ? null : Number(contentStartSelect.value),
  };
  renderRules(lastReport);
  updateFixButton();
}

coverEndSelect.addEventListener('change', onBoundaryChange);
contentStartSelect.addEventListener('change', onBoundaryChange);

shareProfileBtn.addEventListener('click', async () => {
  const profile = profileFromForm();
  const url = `${window.location.origin}${window.location.pathname}#profil=${toBase64Url(profile)}`;
  try {
    await navigator.clipboard.writeText(url);
    profileStatus.textContent = 'Link profil disalin.';
  } catch {
    window.location.hash = `profil=${toBase64Url(profile)}`;
    profileStatus.textContent = 'Salin link dari address bar browser.';
  }
});

// Ubah kolom -> pilihan preset jadi "Profil kustom" dan pindai ulang
// otomatis (debounce) bila dokumen sudah ada. Pengisian formulir oleh
// kode (preset / link profil) tidak memicu event input, jadi tidak
// ikut mengubah pilihan preset.
[
  profileNameInput, profileFontInput, profileSizeInput, profileSpacingInput,
  profileMarginTopInput, profileMarginRightInput, profileMarginBottomInput,
  profileMarginLeftInput, profileHeadingFontInput,
].forEach((input) => {
  const rescan = () => {
    presetSelect.value = 'kustom';
    if (!docFile) return;
    clearTimeout(rescanTimer);
    rescanTimer = setTimeout(scanDocument, 400);
  };
  input.addEventListener('input', rescan);
  input.addEventListener('change', rescan); // dropdown font: 'change' yang pasti jalan
});

initProfileForm();

function showError(message) {
  errorEl.textContent = message;
  errorEl.style.display = 'block';
}

function hideError() {
  errorEl.style.display = 'none';
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function joinCounted(items) {
  return items.map((item) => `${item.value} (${item.count})`).join(', ') || '-';
}

function summaryLines(report) {
  const margins = report.margins_cm
    ? report.margins_cm.map((m) => m.toFixed(1).replace('.', ',')).join('-')
    : '-';
  return [
    `Font isi: ${joinCounted(report.fonts)}`,
    `Font heading: ${joinCounted(report.heading_fonts)}`,
    `Ukuran isi: ${joinCounted(report.sizes)}`,
    `Ukuran heading: ${joinCounted(report.heading_sizes)}`,
    `Spasi: ${joinCounted(report.spacings)}`,
    `Margin: ${margins} cm`,
  ];
}

function listsLine(lists) {
  if (!lists) return '-';
  const parts = [];
  parts.push(lists.toc_entries == null ? 'isi: tidak ada' : `isi: ${lists.toc_entries} entri`);
  for (const table of lists.tables || []) {
    parts.push(`${table.label}: ${table.entries == null ? 'ada' : `${table.entries} entri`}`);
  }
  if (lists.bibliography_entries != null) {
    parts.push(`pustaka: ${lists.bibliography_entries} entri`);
  }
  return parts.join(' · ');
}

// Daftar centang aturan: satu baris per kelompok, dengan status dari
// laporan pindai terakhir (jumlah temuan kelompok itu, atau "Sesuai").
// Penomoran halaman tidak punya temuan: statusnya kesiapan batas.
// Tanpa WASM baru, baris penomoran disembunyikan (fitur belum ada).
function renderRules(report) {
  const current = report || lastReport || { issues: [] };
  rulesList.innerHTML = '';
  RULE_GROUPS.forEach(({ key, label }) => {
    if (key === 'page_numbering' && !wasmSupportsPageNumbering) return;
    const row = document.createElement('p');
    row.style.margin = '6px 0';
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = ruleSelection[key];
    checkbox.addEventListener('change', () => {
      ruleSelection[key] = checkbox.checked;
      if (key === 'page_numbering') {
        pageNumberingSection.style.display = checkbox.checked ? 'block' : 'none';
      }
      updateFixButton();
    });
    const labelEl = document.createElement('label');
    labelEl.style.fontWeight = '700';
    labelEl.appendChild(checkbox);
    labelEl.appendChild(document.createTextNode(` ${label} `));
    const status = document.createElement('span');
    if (key === 'page_numbering') {
      if (pageBounds.coverEnd == null || pageBounds.contentStart == null) {
        status.textContent = '(pilih batas)';
      } else if (!boundsValid()) {
        status.textContent = '(batas tidak valid)';
      } else {
        status.textContent = '(siap)';
      }
    } else {
      const found = current.issues
        .filter((issue) => issue.kind !== 'info' && issue.group === key)
        .reduce((sum, issue) => sum + issue.count, 0);
      status.textContent = found > 0 ? `(${found} temuan)` : '(sesuai)';
    }
    row.appendChild(labelEl);
    row.appendChild(status);
    rulesList.appendChild(row);
  });
  pageNumberingSection.style.display = ruleSelection.page_numbering ? 'block' : 'none';
}

// Tombol perbaiki aktif bila masih ada temuan di kelompok yang
// dicentang, atau penomoran halaman siap diterapkan. Bila penomoran
// dicentang tapi batasnya belum valid, tombol tetap mati: perbaikan
// tanpa batas pasti ditolak mesin, jadi jangan biarkan diklik.
function updateFixButton(report) {
  const current = report || lastReport;
  if (!current) {
    fixBtn.disabled = true;
    return;
  }
  const hasFindings = current.issues.some(
    (issue) => issue.kind !== 'info' && ruleSelection[issue.group],
  );
  const numberingReady = ruleSelection.page_numbering && boundsValid();
  const numberingBlocked = ruleSelection.page_numbering && !numberingReady;
  fixBtn.disabled = numberingBlocked || (!hasFindings && !numberingReady);
}

function renderReport(report) {
  scanSummary.innerHTML = `
    <p><strong>Format:</strong> ${profileFromForm().name}</p>
    <p><strong>Paragraf:</strong> ${report.paragraphs} &middot; <strong>Tabel:</strong> ${report.tables} &middot; <strong>Section:</strong> ${report.sections}</p>
    <p><strong>Font isi:</strong> ${joinCounted(report.fonts)}</p>
    <p><strong>Font heading:</strong> ${joinCounted(report.heading_fonts)}</p>
    <p><strong>Ukuran isi:</strong> ${joinCounted(report.sizes)}</p>
    <p><strong>Ukuran heading:</strong> ${joinCounted(report.heading_sizes)}</p>
    <p><strong>Spasi:</strong> ${joinCounted(report.spacings)}</p>
    <p><strong>Margin:</strong> ${report.margins_cm ? report.margins_cm.map((m) => m.toFixed(1).replace('.', ',')).join('-') : '-'} cm</p>
    <p><strong>Daftar:</strong> ${listsLine(report.lists)}</p>
  `;
  issueList.innerHTML = '';
  if (!report.issues.length) {
    issueList.innerHTML = '<li>Tidak ada masalah. Dokumen sudah sesuai profil.</li>';
  } else {
    report.issues.forEach((issue) => {
      const item = document.createElement('li');
      item.textContent = issue.kind === 'info' ? `Catatan: ${issue.message}` : issue.message;
      issueList.appendChild(item);
    });
  }
  renderRules(report);
  updateFixButton(report);
}

function renderCompare(before, after) {
  const beforeLines = summaryLines(before);
  const afterLines = summaryLines(after);
  compareBody.innerHTML = `
    <p><strong>Sebelum:</strong><br>${beforeLines.join('<br>')}</p>
    <p><strong>Sesudah:</strong><br>${afterLines.join('<br>')}</p>
  `;
  compareSection.style.display = 'block';
}

async function scanDocument() {
  // Jangan ganggu perbaikan yang sedang jalan: pindai di sini menimpa
  // loading, menonaktifkan tombol, dan menyembunyikan hasil perbaikan.
  if (fixing) return;
  hideError();
  loadingEl.textContent = 'Memindai dokumen...';
  loadingEl.style.display = 'block';
  fixBtn.disabled = true;
  try {
    const report = await scanActive(docFile.data);
    // Perbaikan bisa saja mulai saat pindai ini menunggu WASM: kalau
    // begitu, jangan sentuh tampilan sama sekali.
    if (fixing) return;
    lastReport = report;
    renderReport(report);
    loadPageParagraphs();
    scanSection.style.display = 'block';
    resultSection.style.display = 'none';
  } catch (error) {
    showError(`Gagal memindai dokumen: ${String(error).replace(/\.+$/, '')}. Coba muat ulang halaman.`);
  } finally {
    loadingEl.style.display = 'none';
  }
}

function isDocxFile(file) {
  // Sebagian pemilih file di Android mengembalikan type kosong; cek
  // ekstensi sebagai cadangan agar dokumen tetap diterima.
  return file.type === DOCX_TYPE || /\.docx$/i.test(file.name);
}

async function addFile(fileList) {
  hideError();
  const file = Array.from(fileList).find(isDocxFile);
  if (!file) {
    showError('Pilih file .docx.');
    return;
  }
  const buffer = await file.arrayBuffer();
  docFile = { name: file.name, data: new Uint8Array(buffer) };
  // Dokumen baru: batas penomoran dokumen lama tidak berlaku lagi.
  pageBounds = { coverEnd: null, contentStart: null };
  coverEndSelect.value = '';
  contentStartSelect.value = '';
  fileNameEl.textContent = file.name;
  fileSizeEl.textContent = `(${formatSize(file.size)})`;
  filenameInput.value = file.name.replace(/\.docx$/i, '') + '-rapi';
  fileSection.style.display = 'block';
  profileSection.style.display = 'block';
  await scanDocument();
}

fileInput.addEventListener('change', () => {
  addFile(fileInput.files);
  // Kosongkan value supaya file yang sama bisa dipilih lagi.
  fileInput.value = '';
});
dropArea.addEventListener('dragover', (event) => {
  event.preventDefault();
  dropArea.classList.add('highlight');
});
dropArea.addEventListener('dragleave', () => dropArea.classList.remove('highlight'));
dropArea.addEventListener('drop', (event) => {
  event.preventDefault();
  dropArea.classList.remove('highlight');
  addFile(event.dataTransfer.files);
});
removeFileBtn.addEventListener('click', () => {
  docFile = null;
  fileSection.style.display = 'none';
  profileSection.style.display = 'none';
  scanSection.style.display = 'none';
  resultSection.style.display = 'none';
  fixBtn.disabled = true;
});

// Nama file bisa diubah setelah dokumen jadi: perbarui nama unduhan
// langsung, tanpa harus memperbaiki ulang.
filenameInput.addEventListener('input', () => {
  if (!docUrl) return;
  const name = (filenameInput.value.trim() || 'dokumen-rapi').replace(/[\\/:*?"<>|]/g, '-');
  downloadLink.download = `${name}.docx`;
});

fixBtn.addEventListener('click', async () => {
  if (!docFile) return;
  // Batalkan pindai otomatis yang masih menunggu: kalau ia jalan di
  // tengah perbaikan, hasilnya menimpa tampilan perbaikan.
  clearTimeout(rescanTimer);
  fixing = true;
  hideError();
  fixBtn.disabled = true;
  loadingEl.textContent = 'Memperbaiki dokumen...';
  loadingEl.style.display = 'block';
  try {
    const { docx_text } = await loadWasm();
    const textBefore = docx_text(docFile.data);
    const fixedBytes = await fixActive(docFile.data);
    const blob = new Blob([fixedBytes], { type: DOCX_TYPE });
    if (docUrl) URL.revokeObjectURL(docUrl);
    docUrl = URL.createObjectURL(blob);
    const name = (filenameInput.value.trim() || 'dokumen-rapi').replace(/[\\/:*?"<>|]/g, '-');
    downloadLink.href = docUrl;
    downloadLink.download = `${name}.docx`;
    // Pindai ulang hasil perbaikan: harusnya tidak ada masalah tersisa.
    const report = await scanActive(fixedBytes);
    const remaining = report.issues.filter((issue) => issue.kind !== 'info');
    rescanResult.textContent = remaining.length === 0
      ? 'Hasil pindai ulang: 0 masalah tersisa.'
      : `Hasil pindai ulang: ${remaining.length} masalah tersisa.`;
    // Peringatan nomor halaman hanya bila dokumen hasil perbaikan
    // punya daftar isi: angkanya perkiraan, bukan angka eksak.
    pageNumberWarning.style.display = report.lists && report.lists.toc_entries != null
      ? 'block'
      : 'none';
    remainingList.innerHTML = '';
    report.issues.forEach((issue) => {
      const item = document.createElement('li');
      item.textContent = issue.kind === 'info' ? `Catatan: ${issue.message}` : issue.message;
      remainingList.appendChild(item);
    });
    // Banding sebelum/sesudah: ringkasan format + bukti teks isi tidak berubah.
    if (lastReport) renderCompare(lastReport, report);
    const textAfter = docx_text(fixedBytes);
    // Teks isi identik kata per kata; daftar isi, tabel, dan gambar
    // dibuat atau dibangun ulang bila ada (bukan bagian dari teks isi).
    textVerdict.textContent = textBefore === textAfter
      ? 'Teks isi identik kata per kata.' + (ruleSelection.lists ? ' Daftar isi, tabel, dan gambar dibuat atau dibangun ulang bila ada.' : '')
      : 'Perhatian: teks dokumen berubah saat perbaikan. Periksa hasil unduhan.';
    // Tampilkan hasil pindai ulang sebagai laporan saat ini.
    renderReport(report);
    resultSection.style.display = 'block';
    resultSection.scrollIntoView({ behavior: 'smooth' });
  } catch (error) {
    showError(`Gagal memperbaiki dokumen: ${String(error).replace(/\.+$/, '')}. Coba muat ulang halaman.`);
    updateFixButton();
  } finally {
    fixing = false;
    loadingEl.style.display = 'none';
  }
});
