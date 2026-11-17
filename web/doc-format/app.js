// Doc Format: shell web untuk tulung-core (WASM).
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
const presetSection = document.getElementById('preset-section');
const scanSection = document.getElementById('scan-section');
const scanSummary = document.getElementById('scan-summary');
const issueList = document.getElementById('issue-list');
const selectAllBtn = document.getElementById('select-all');
const selectNoneBtn = document.getElementById('select-none');
const fixBtn = document.getElementById('fix-btn');
const loadingEl = document.getElementById('loading');
const errorEl = document.getElementById('error');
const resultSection = document.getElementById('result-section');
const filenameInput = document.getElementById('filename');
const downloadLink = document.getElementById('download-link');
const rescanResult = document.getElementById('rescan-result');
const remainingList = document.getElementById('remaining-list');
const compareSection = document.getElementById('compare-section');
const compareBody = document.getElementById('compare-body');
const textVerdict = document.getElementById('text-verdict');

const DOCX_TYPE = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';

let docFile = null; // { name, data: Uint8Array }
let wasmPromise = null;
let docUrl = null;
let lastReport = null; // Laporan pindai sebelum perbaikan (untuk banding).
const preset = 'umum'; // Satu standar universal, bukan per kampus.

function loadWasm() {
  if (!wasmPromise) {
    wasmPromise = import('../pkg/tulung_core.js').then(async (mod) => {
      await mod.default();
      return mod;
    });
  }
  return wasmPromise;
}

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

function renderReport(report) {
  scanSummary.innerHTML = `
    <p><strong>Paragraf:</strong> ${report.paragraphs} &middot; <strong>Tabel:</strong> ${report.tables} &middot; <strong>Section:</strong> ${report.sections}</p>
    <p><strong>Font:</strong> ${joinCounted(report.fonts)}</p>
    <p><strong>Ukuran:</strong> ${joinCounted(report.sizes)}</p>
    <p><strong>Spasi:</strong> ${joinCounted(report.spacings)}</p>
    <p><strong>Margin:</strong> ${report.margins_cm ? report.margins_cm.map((m) => m.toFixed(1).replace('.', ',')).join('-') : '-'} cm</p>
  `;
  issueList.innerHTML = '';
  const actionable = report.issues.filter((issue) => issue.kind !== 'info');
  if (!report.issues.length) {
    issueList.innerHTML = '<li>Tidak ada masalah. Dokumen sudah sesuai preset.</li>';
  } else {
    report.issues.forEach((issue) => {
      const item = document.createElement('li');
      if (issue.kind === 'info') {
        item.textContent = `Catatan: ${issue.message}`;
      } else {
        const label = document.createElement('label');
        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.dataset.id = issue.id;
        checkbox.checked = true;
        checkbox.addEventListener('change', updateFixButton);
        label.appendChild(checkbox);
        label.appendChild(document.createTextNode(` ${issue.message}`));
        item.appendChild(label);
      }
      issueList.appendChild(item);
    });
  }
  updateFixButton();
}

function selectedIds() {
  return Array.from(issueList.querySelectorAll('input[type="checkbox"]:checked'))
    .map((box) => box.dataset.id);
}

function updateFixButton() {
  fixBtn.disabled = selectedIds().length === 0;
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

selectAllBtn.addEventListener('click', () => {
  issueList.querySelectorAll('input[type="checkbox"]').forEach((box) => { box.checked = true; });
  updateFixButton();
});
selectNoneBtn.addEventListener('click', () => {
  issueList.querySelectorAll('input[type="checkbox"]').forEach((box) => { box.checked = false; });
  updateFixButton();
});

async function scanDocument() {
  hideError();
  loadingEl.textContent = 'Memindai dokumen...';
  loadingEl.style.display = 'block';
  fixBtn.disabled = true;
  try {
    const { scan_docx } = await loadWasm();
    const report = JSON.parse(scan_docx(docFile.data, preset));
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
  presetSection.style.display = 'block';
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
  presetSection.style.display = 'none';
  scanSection.style.display = 'none';
  resultSection.style.display = 'none';
  fixBtn.disabled = true;
});

fixBtn.addEventListener('click', async () => {
  if (!docFile) return;
  hideError();
  fixBtn.disabled = true;
  loadingEl.textContent = 'Memperbaiki dokumen...';
  loadingEl.style.display = 'block';
  try {
    const { fix_docx, scan_docx, docx_text } = await loadWasm();
    const textBefore = docx_text(docFile.data);
    const fixedBytes = fix_docx(docFile.data, preset, JSON.stringify(selectedIds()));
    const blob = new Blob([fixedBytes], { type: DOCX_TYPE });
    if (docUrl) URL.revokeObjectURL(docUrl);
    docUrl = URL.createObjectURL(blob);
    const name = (filenameInput.value.trim() || 'dokumen-rapi').replace(/[\\/:*?"<>|]/g, '-');
    downloadLink.href = docUrl;
    downloadLink.download = `${name}.docx`;
    // Pindai ulang hasil perbaikan: harusnya tidak ada masalah tersisa.
    const report = JSON.parse(scan_docx(fixedBytes, preset));
    const remaining = report.issues.filter((issue) => issue.kind !== 'info');
    rescanResult.textContent = remaining.length === 0
      ? 'Hasil pindai ulang: 0 masalah tersisa.'
      : `Hasil pindai ulang: ${remaining.length} masalah tersisa.`;
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
    updateFixButton();
  } finally {
    loadingEl.style.display = 'none';
  }
});
