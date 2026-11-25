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

let images = []; // { name, data: Uint8Array, url: string, rotation: number }
let wasmPromise = null;
let pdfUrl = null;

// Opsi halaman, default: potret, A4, tanpa margin.
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

// Putar gambar 90 derajat searah jarum jam. Rotasi relatif terhadap
// tampilan (orientasi EXIF sudah diterapkan browser di bitmap), sama
// seperti urutan yang dikerjakan WASM: EXIF dulu, baru rotasi manual.
// Pratinjau digambar ulang lewat canvas supaya yang terlihat sama
// dengan hasil PDF; byte asli tidak diubah, rotasinya dikirim ke WASM.
async function rotateImage(index) {
  const img = images[index];
  img.rotation = (img.rotation + 90) % 360;
  try {
    const bitmap = await createImageBitmap(new Blob([img.data]));
    const swap = img.rotation % 180 !== 0;
    const canvas = document.createElement('canvas');
    canvas.width = swap ? bitmap.height : bitmap.width;
    canvas.height = swap ? bitmap.width : bitmap.height;
    const ctx = canvas.getContext('2d');
    ctx.translate(canvas.width / 2, canvas.height / 2);
    ctx.rotate((img.rotation * Math.PI) / 180);
    ctx.drawImage(bitmap, -bitmap.width / 2, -bitmap.height / 2);
    bitmap.close();
    const blob = await new Promise((resolve) =>
      canvas.toBlob(resolve, 'image/jpeg', 0.85));
    if (blob) {
      URL.revokeObjectURL(img.url);
      img.url = URL.createObjectURL(blob);
    }
  } catch {
    // Pratinjau gagal digambar ulang; rotasi tetap dikirim ke WASM,
    // jadi hasil PDF tetap benar.
  }
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
      <button class="rotate" title="Putar 90 derajat">&#8635;</button>
      <button class="move move-up" title="Geser ke atas" ${index === 0 ? 'disabled' : ''}>&uarr;</button>
      <button class="move move-down" title="Geser ke bawah" ${index === images.length - 1 ? 'disabled' : ''}>&darr;</button>
    `;
    item.querySelector('.remove').addEventListener('click', () => {
      URL.revokeObjectURL(img.url);
      images.splice(index, 1);
      renderPreviews();
    });
    item.querySelector('.rotate').addEventListener('click', () => rotateImage(index));
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
      rotation: 0,
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

function outputName() {
  return (filenameInput.value.trim() || 'tulung-gambar-ke-pdf')
    .replace(/[\\/:*?"<>|]/g, '-');
}

// Nama file bisa diubah setelah PDF jadi: perbarui nama unduhan
// langsung, tanpa harus membuat PDF ulang.
filenameInput.addEventListener('input', () => {
  if (!pdfUrl) return;
  downloadLink.download = `${outputName()}.pdf`;
});

convertBtn.addEventListener('click', async () => {
  hideError();
  convertBtn.disabled = true;
  loadingEl.style.display = 'block';
  try {
    const { images_to_pdf } = await loadWasm();
    const pdfBytes = images_to_pdf(
      images.map((img) => img.data),
      images.map((img) => img.rotation),
      options.orientation,
      options.pageSize,
      options.margin,
    );
    const blob = new Blob([pdfBytes], { type: 'application/pdf' });
    if (pdfUrl) URL.revokeObjectURL(pdfUrl);
    pdfUrl = URL.createObjectURL(blob);
    downloadLink.href = pdfUrl;
    downloadLink.download = `${outputName()}.pdf`;
    pdfPreview.src = pdfUrl;
    resultSection.style.display = 'block';
    resultSection.scrollIntoView({ behavior: 'smooth' });
  } catch (error) {
    showError(`Gagal membuat PDF: ${String(error).replace(/\.+$/, '')}. Coba muat ulang halaman.`);
  } finally {
    loadingEl.style.display = 'none';
    convertBtn.disabled = images.length === 0;
  }
});
