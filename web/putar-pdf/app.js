// Putar PDF: shell web untuk tulung-pdf (WASM).
// Alur: baca file sebagai byte -> kirim ke putar_pdf() di WASM
// beserta derajat dan rentang halaman -> terima byte PDF hasil
// -> unduh + pratinjau. Tidak ada server.

const dropArea = document.getElementById('drop-area');
const fileInput = document.getElementById('file-input');
const fileSection = document.getElementById('file-section');
const fileInfo = document.getElementById('file-info');
const pagesInput = document.getElementById('pages');
const convertBtn = document.getElementById('convert-btn');
const loadingEl = document.getElementById('loading');
const errorEl = document.getElementById('error');
const resultSection = document.getElementById('result-section');
const filenameInput = document.getElementById('filename');
const downloadLink = document.getElementById('download-link');
const pdfPreview = document.getElementById('pdf-preview');

let pdf = null; // { name, data: Uint8Array }
let wasmPromise = null;
let pdfUrl = null;
let degrees = 90;

document.querySelectorAll('.option-buttons').forEach((group) => {
  group.addEventListener('click', (event) => {
    const button = event.target.closest('.option-btn');
    if (!button) return;
    group.querySelectorAll('.option-btn').forEach((b) => b.classList.remove('selected'));
    button.classList.add('selected');
    degrees = Number(button.dataset.value);
  });
});

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

function isPdfFile(file) {
  return file.type === 'application/pdf' || /\.pdf$/i.test(file.name);
}

async function addFile(fileListInput) {
  hideError();
  const picked = Array.from(fileListInput).filter(isPdfFile);
  if (!picked.length) {
    showError('Pilih file PDF.');
    return;
  }
  const file = picked[0];
  const buffer = await file.arrayBuffer();
  pdf = { name: file.name, data: new Uint8Array(buffer) };
  fileInfo.textContent = `${pdf.name} (${formatSize(pdf.data.length)})`;
  fileSection.style.display = 'block';
  convertBtn.disabled = false;
  resultSection.style.display = 'none';
}

fileInput.addEventListener('change', () => {
  addFile(fileInput.files);
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

function outputName() {
  return (filenameInput.value.trim() || 'tulung-putar-pdf')
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
    const { putar_pdf } = await loadWasm();
    const pdfBytes = putar_pdf(pdf.data, degrees, pagesInput.value.trim());
    const blob = new Blob([pdfBytes], { type: 'application/pdf' });
    if (pdfUrl) URL.revokeObjectURL(pdfUrl);
    pdfUrl = URL.createObjectURL(blob);
    downloadLink.href = pdfUrl;
    downloadLink.download = `${outputName()}.pdf`;
    pdfPreview.src = pdfUrl;
    resultSection.style.display = 'block';
    resultSection.scrollIntoView({ behavior: 'smooth' });
  } catch (error) {
    showError(`Gagal memutar PDF: ${String(error).replace(/\.+$/, '')}. Coba muat ulang halaman.`);
  } finally {
    loadingEl.style.display = 'none';
    convertBtn.disabled = !pdf;
  }
});
