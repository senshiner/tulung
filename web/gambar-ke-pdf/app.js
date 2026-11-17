// Gambar ke PDF: shell web untuk tulung-core (WASM).
// Alur: baca file sebagai byte -> kirim ke images_to_pdf() di WASM
// -> terima byte PDF -> unduh + pratinjau. Tidak ada server.

import init, { images_to_pdf } from '../pkg/tulung_core.js';

const dropArea = document.getElementById('drop-area');
const fileInput = document.getElementById('file-input');
const previewSection = document.getElementById('preview-section');
const previewGrid = document.getElementById('preview-grid');
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

let images = []; // { name, data: Uint8Array, url: string }
let wasmReady = init();
let pdfUrl = null;

function showError(message) {
  errorEl.textContent = message;
  errorEl.style.display = 'block';
}

function hideError() {
  errorEl.style.display = 'none';
}

function renderPreviews() {
  previewGrid.innerHTML = '';
  images.forEach((img, index) => {
    const item = document.createElement('div');
    item.className = 'preview-item brutal-border';
    item.draggable = true;
    item.dataset.index = index;
    item.innerHTML = `
      <img src="${img.url}" alt="${img.name}">
      <p>${index + 1}. ${img.name}</p>
      <button class="remove" title="Hapus">x</button>
    `;
    item.querySelector('.remove').addEventListener('click', () => {
      URL.revokeObjectURL(img.url);
      images.splice(index, 1);
      renderPreviews();
    });
    item.addEventListener('dragstart', (event) => {
      event.dataTransfer.setData('text/plain', index);
    });
    item.addEventListener('dragover', (event) => event.preventDefault());
    item.addEventListener('drop', (event) => {
      event.preventDefault();
      const from = Number(event.dataTransfer.getData('text/plain'));
      const [moved] = images.splice(from, 1);
      images.splice(index, 0, moved);
      renderPreviews();
    });
    previewGrid.appendChild(item);
  });
  countEl.textContent = images.length;
  summaryEl.textContent = `${images.length} gambar dipilih`;
  previewSection.style.display = images.length ? 'block' : 'none';
  convertBtn.disabled = images.length === 0;
  if (!images.length) resultSection.style.display = 'none';
}

async function addFiles(fileList) {
  hideError();
  const files = Array.from(fileList).filter((file) => file.type.startsWith('image/'));
  if (!files.length) {
    showError('Pilih file gambar (JPG, PNG, WebP, BMP, GIF).');
    return;
  }
  for (const file of files) {
    const buffer = await file.arrayBuffer();
    images.push({
      name: file.name,
      data: new Uint8Array(buffer),
      url: URL.createObjectURL(file),
    });
  }
  renderPreviews();
}

dropArea.addEventListener('click', (event) => {
  if (event.target === dropArea || event.target.closest('p')) fileInput.click();
});
fileInput.addEventListener('change', () => addFiles(fileInput.files));
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
  images.forEach((img) => URL.revokeObjectURL(img.url));
  images = [];
  renderPreviews();
});

convertBtn.addEventListener('click', async () => {
  hideError();
  convertBtn.disabled = true;
  loadingEl.style.display = 'block';
  try {
    await wasmReady;
    const pdfBytes = images_to_pdf(images.map((img) => img.data));
    const blob = new Blob([pdfBytes], { type: 'application/pdf' });
    if (pdfUrl) URL.revokeObjectURL(pdfUrl);
    pdfUrl = URL.createObjectURL(blob);
    const name = (filenameInput.value.trim() || 'tulung-gambar-ke-pdf').replace(/[\\/:*?"<>|]/g, '-');
    downloadLink.href = pdfUrl;
    downloadLink.download = `${name}.pdf`;
    pdfPreview.src = pdfUrl;
    resultSection.style.display = 'block';
    resultSection.scrollIntoView({ behavior: 'smooth' });
  } catch (error) {
    showError(`Gagal membuat PDF: ${error}`);
  } finally {
    loadingEl.style.display = 'none';
    convertBtn.disabled = images.length === 0;
  }
});
