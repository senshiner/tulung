// Doc Format: shell web untuk tulung-docx (WASM).
// Alur: baca .docx sebagai byte -> pindai dengan scan_docx() di WASM
// -> tampilkan daftar masalah -> perbaiki dengan fix_docx() -> unduh
// + pindai ulang hasil perbaikan. Tidak ada server; isi dokumen
// tidak berubah, hanya formatnya.
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
    const { default_profile } = await loadWasm();
    const defaults = JSON.parse(default_profile());
    const shared = profileFromHash();
    if (shared) {
      fillProfileForm(shared);
      profileStatus.textContent = `Profil dari link: ${shared.name}.`;
    } else {
      fillProfileForm(defaults);
    }
  } catch {
    // Formulir tetap bisa dipakai tanpa WASM; angka default ditulis
    // ulang saat pemindaian pertama berhasil.
  }
}

// Format yang dipakai pindai dan perbaikan selalu dibaca dari kolom
// formulir saat itu juga: tidak ada mode standar terpisah.
async function scanActive(bytes) {
  const wasm = await loadWasm();
  return JSON.parse(wasm.scan_docx_profile(bytes, JSON.stringify(profileFromForm())));
}

async function fixActive(bytes) {
  const wasm = await loadWasm();
  return wasm.fix_docx_profile(bytes, JSON.stringify(profileFromForm()));
}

shareProfileBtn.addEventListener('click', async () => {
  const profile = profileFromForm();
  const url = `${window.location.origin}${window.location.pathname}#profil=${toBase64Url(profile)}`;
  try {
    await navigator.clipboard.writeText(url);
    profileStatus.textContent = 'Link profil disalin. Bagikan ke seangkatan, mereka langsung pakai format ini.';
  } catch {
    window.location.hash = `profil=${toBase64Url(profile)}`;
    profileStatus.textContent = 'Salin link dari address bar browser.';
  }
});

// Ubah kolom -> pindai ulang otomatis (debounce) bila dokumen sudah ada.
[
  profileNameInput, profileFontInput, profileSizeInput, profileSpacingInput,
  profileMarginTopInput, profileMarginRightInput, profileMarginBottomInput,
  profileMarginLeftInput, profileHeadingFontInput,
].forEach((input) => {
  const rescan = () => {
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
    `Font: ${joinCounted(report.fonts)}`,
    `Ukuran: ${joinCounted(report.sizes)}`,
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

function renderReport(report) {
  scanSummary.innerHTML = `
    <p><strong>Format:</strong> ${profileFromForm().name}</p>
    <p><strong>Paragraf:</strong> ${report.paragraphs} &middot; <strong>Tabel:</strong> ${report.tables} &middot; <strong>Section:</strong> ${report.sections}</p>
    <p><strong>Font:</strong> ${joinCounted(report.fonts)}</p>
    <p><strong>Ukuran:</strong> ${joinCounted(report.sizes)}</p>
    <p><strong>Spasi:</strong> ${joinCounted(report.spacings)}</p>
    <p><strong>Margin:</strong> ${report.margins_cm ? report.margins_cm.map((m) => m.toFixed(1).replace('.', ',')).join('-') : '-'} cm</p>
    <p><strong>Daftar:</strong> ${listsLine(report.lists)}</p>
  `;
  issueList.innerHTML = '';
  const actionable = report.issues.filter((issue) => issue.kind !== 'info');
  if (!report.issues.length) {
    issueList.innerHTML = '<li>Tidak ada masalah. Dokumen sudah sesuai profil.</li>';
  } else {
    report.issues.forEach((issue) => {
      const item = document.createElement('li');
      item.textContent = issue.kind === 'info' ? `Catatan: ${issue.message}` : issue.message;
      issueList.appendChild(item);
    });
  }
  fixBtn.disabled = actionable.length === 0;
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
    scanSection.style.display = 'block';
    resultSection.style.display = 'none';
  } catch (error) {
    showError(`Gagal memindai dokumen: ${error}. Coba muat ulang halaman.`);
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
    // Banding sebelum/sesudah: ringkasan format + bukti isi tidak berubah.
    if (lastReport) renderCompare(lastReport, report);
    const textAfter = docx_text(fixedBytes);
    textVerdict.textContent = textBefore === textAfter
      ? 'Isi identik kata per kata: teks sebelum dan sesudah perbaikan sama.'
      : 'Perhatian: teks dokumen berubah saat perbaikan. Periksa hasil unduhan.';
    // Tampilkan hasil pindai ulang sebagai laporan saat ini.
    renderReport(report);
    resultSection.style.display = 'block';
    resultSection.scrollIntoView({ behavior: 'smooth' });
  } catch (error) {
    showError(`Gagal memperbaiki dokumen: ${error}. Coba muat ulang halaman.`);
    fixBtn.disabled = false;
  } finally {
    fixing = false;
    loadingEl.style.display = 'none';
  }
});
