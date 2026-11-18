// Gambar ke PDF: shell web untuk tulung-pdf (WASM).
// Alur: baca file sebagai byte -> kirim ke images_to_pdf() di WASM
// -> terima byte PDF -> unduh + pratinjau. Tidak ada server.
//
// WASM dimuat dinamis di dalam handler konversi, bukan import statis di
// atas: kalau modul WASM gagal dimuat, pemilih gambar tetap jalan dan
// pengguna dapat pesan error yang jelas, bukan halaman yang diam saja.

const dropArea = document.getElementById('drop-area');
const fileInput = document.getElementById('file-input');
const previewSection = document.getElementById('preview-section');
const optionsSection = document.getElementById('options-section');
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

const IMAGE_EXTENSIONS = /\.(jpe?g|png|webp|bmp|gif)$/i;

let images = []; // { name, data: Uint8Array, url: string }
let wasmPromise = null;
let pdfUrl = null;

// Opsi halaman, default mengikuti iLovePDF: potret, A4, tanpa margin.
const options = { orientation: 'portrait', pageSize: 'a4', margin: 'none' };

document.querySelectorAll('.option-buttons').forEach((group) => {
  group.addEventListener('click', (event) => {
    const button = event.target.closest('.option-btn');
    if (!button) return;
    group.querySelectorAll('.option-btn').forEach((b) => b.classList.remove('selected'));
    button.classList.add('selected');
    options[group.dataset.option] = button.dataset.value;
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

function moveImage(from, to) {
  if (to < 0 || to >= images.length) return;
  const [moved] = images.splice(from, 1);
  images.splice(to, 0, moved);
  renderPreviews();
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
      <button class="move move-up" title="Geser ke atas" ${index === 0 ? 'disabled' : ''}>&uarr;</button>
      <button class="move move-down" title="Geser ke bawah" ${index === images.length - 1 ? 'disabled' : ''}>&darr;</button>
    `;
    item.querySelector('.remove').addEventListener('click', () => {
      URL.revokeObjectURL(img.url);
      images.splice(index, 1);
      renderPreviews();
    });
    item.querySelector('.move-up').addEventListener('click', () => moveImage(index, index - 1));
    item.querySelector('.move-down').addEventListener('click', () => moveImage(index, index + 1));
    item.addEventListener('dragstart', (event) => {
      event.dataTransfer.setData('text/plain', index);
    });
    item.addEventListener('dragover', (event) => event.preventDefault());
    item.addEventListener('drop', (event) => {
      event.preventDefault();
      const from = Number(event.dataTransfer.getData('text/plain'));
      moveImage(from, index);
    });
    previewGrid.appendChild(item);
  });
  countEl.textContent = images.length;
  summaryEl.textContent = `${images.length} gambar`;
  previewSection.style.display = images.length ? 'block' : 'none';
  optionsSection.style.display = images.length ? 'block' : 'none';
  convertBtn.disabled = images.length === 0;
  if (!images.length) resultSection.style.display = 'none';
}

function isImageFile(file) {
  // Sebagian pemilih file di Android mengembalikan type kosong; cek
  // ekstensi sebagai cadangan agar gambar tetap diterima.
  return file.type.startsWith('image/') || IMAGE_EXTENSIONS.test(file.name);
}

async function addFiles(fileList) {
  hideError();
  const files = Array.from(fileList).filter(isImageFile);
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

fileInput.addEventListener('change', () => {
  addFiles(fileInput.files);
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
    const { images_to_pdf } = await loadWasm();
    const pdfBytes = images_to_pdf(
      images.map((img) => img.data),
      options.orientation,
      options.pageSize,
      options.margin,
    );
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
    showError(`Gagal membuat PDF: ${error}. Coba muat ulang halaman.`);
  } finally {
    loadingEl.style.display = 'none';
    convertBtn.disabled = images.length === 0;
  }
});
