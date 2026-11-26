// Gabung PDF: shell web untuk tulung-pdf (WASM).
// Alur: baca tiap file sebagai byte -> kirim ke gabung_pdf() di WASM
// -> terima byte PDF gabungan -> unduh + pratinjau. Tidak ada server.
//
// WASM dimuat dinamis di dalam handler konversi, bukan import statis
// di atas, supaya halaman tetap merespons saat modul WASM gagal dimuat.

const dropArea = document.getElementById('drop-area');
const fileInput = document.getElementById('file-input');
const listSection = document.getElementById('list-section');
const fileList = document.getElementById('file-list');
const countEl = document.getElementById('count');
const summaryEl = document.getElementById('summary');
const clearAllBtn = document.getElementById('clear-all');
const convertBtn = document.getElementById('convert-btn');
const loadingEl = document.getElementById('loading');
const errorEl = document.getElementById('error');
const resultSection = document.getElementById('result-section');
const filenameInput = document.getElementById('filename');
const downloadLink = document.getElementById('download-link');
const pdfPreview = document.getElementById('pdf-preview');

let files = []; // { name, data: Uint8Array }
let wasmPromise = null;
let pdfUrl = null;

function loadWasm() {
  if (!wasmPromise) {
    wasmPromise = import('../pkg-pdf/tulung_pdf.js').then(async (mod) => {
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
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function moveFile(from, to) {
  if (to < 0 || to >= files.length) return;
  const [moved] = files.splice(from, 1);
  files.splice(to, 0, moved);
  renderList();
}

function renderList() {
  fileList.innerHTML = '';
  files.forEach((file, index) => {
    const row = document.createElement('div');
    row.className = 'brutal-border';
    row.style.cssText = 'display:flex; align-items:center; gap:8px; padding:10px 12px; margin-top:10px; background:#fff;';
    row.innerHTML = `
      <span style="flex:1; font-weight:700; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">${index + 1}. ${file.name}</span>
      <span>${formatSize(file.data.length)}</span>
      <button class="move-up brutal-border brutal-btn" title="Geser ke atas" style="padding:2px 8px; font-weight:700;" ${index === 0 ? 'disabled' : ''}>&uarr;</button>
      <button class="move-down brutal-border brutal-btn" title="Geser ke bawah" style="padding:2px 8px; font-weight:700;" ${index === files.length - 1 ? 'disabled' : ''}>&darr;</button>
      <button class="remove brutal-border brutal-btn" title="Hapus" style="padding:2px 8px; font-weight:700; background:#ffcccc;">x</button>
    `;
    row.querySelector('.remove').addEventListener('click', () => {
      files.splice(index, 1);
      renderList();
    });
    row.querySelector('.move-up').addEventListener('click', () => moveFile(index, index - 1));
    row.querySelector('.move-down').addEventListener('click', () => moveFile(index, index + 1));
    fileList.appendChild(row);
  });
  countEl.textContent = files.length;
  const total = files.reduce((sum, file) => sum + file.data.length, 0);
  summaryEl.textContent = `${files.length} file, total ${formatSize(total)}`;
  listSection.style.display = files.length ? 'block' : 'none';
  convertBtn.disabled = files.length === 0;
  if (!files.length) resultSection.style.display = 'none';
}

function isPdfFile(file) {
  // Sebagian pemilih file di Android mengembalikan type kosong; cek
  // ekstensi sebagai cadangan.
  return file.type === 'application/pdf' || /\.pdf$/i.test(file.name);
}

async function addFiles(fileListInput) {
  hideError();
  const picked = Array.from(fileListInput).filter(isPdfFile);
  if (!picked.length) {
    showError('Pilih file PDF.');
    return;
  }
  for (const file of picked) {
    const buffer = await file.arrayBuffer();
    files.push({ name: file.name, data: new Uint8Array(buffer) });
  }
  renderList();
}

fileInput.addEventListener('change', () => {
  addFiles(fileInput.files);
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
  addFiles(event.dataTransfer.files);
});
clearAllBtn.addEventListener('click', () => {
  files = [];
  renderList();
});

function outputName() {
  return (filenameInput.value.trim() || 'tulung-gabung-pdf')
    .replace(/[\\/:*?"<>|]/g, '-');
}

filenameInput.addEventListener('input', () => {
  if (!pdfUrl) return;
  downloadLink.download = `${outputName()}.pdf`;
});

convertBtn.addEventListener('click', async () => {
  hideError();
  convertBtn.disabled = true;
  loadingEl.style.display = 'block';
  try {
    const { gabung_pdf } = await loadWasm();
    const pdfBytes = gabung_pdf(files.map((file) => file.data));
    const blob = new Blob([pdfBytes], { type: 'application/pdf' });
    if (pdfUrl) URL.revokeObjectURL(pdfUrl);
    pdfUrl = URL.createObjectURL(blob);
    downloadLink.href = pdfUrl;
    downloadLink.download = `${outputName()}.pdf`;
    pdfPreview.src = pdfUrl;
    resultSection.style.display = 'block';
    resultSection.scrollIntoView({ behavior: 'smooth' });
  } catch (error) {
    showError(`Gagal menggabungkan PDF: ${String(error).replace(/\.+$/, '')}. Coba muat ulang halaman.`);
  } finally {
    loadingEl.style.display = 'none';
    convertBtn.disabled = files.length === 0;
  }
});
