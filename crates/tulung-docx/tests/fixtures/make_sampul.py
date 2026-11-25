"""Buat fixture sampul*.docx untuk tes perlindungan sampul dan
keterangan gambar/tabel Doc Format.

- sampul.docx: sampul 3 paragraf (judul Arial 16 bold tengah,
  dua baris Arial 12 tengah, spasi 1,0) diakhiri page break di run
  terakhir, lalu paragraf isi Calibri 11, paragraf bergaya Caption
  (Calibri 10 miring, tengah), paragraf teks keterangan tanpa gaya
  khusus ("Gambar 2.1 Contoh diagram", Calibri 10 miring), dan satu
  paragraf isi lagi.
- sampul_pbb.docx: sampul 2 paragraf; paragraf isi pertama memakai
  pageBreakBefore.
- sampul_sect.docx: sampul 2 paragraf di section sendiri (section
  break), lalu paragraf isi di section kedua.

Jalankan: python3 make_sampul.py -> menulis ketiga .docx di folder ini.
"""
import docx
from docx.enum.text import WD_ALIGN_PARAGRAPH, WD_BREAK
from docx.shared import Pt, Cm

BODY_TEXT_1 = "Paragraf isi pertama setelah sampul."
BODY_TEXT_2 = "Paragraf isi kedua setelah keterangan."


def cover_title(doc):
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    p.paragraph_format.line_spacing = 1.0
    r = p.add_run("JUDUL SKRIPSI CONTOH")
    r.font.name = "Arial"
    r.font.size = Pt(16)
    r.bold = True
    return p


def cover_line(doc, text, bold=False):
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    p.paragraph_format.line_spacing = 1.0
    r = p.add_run(text)
    r.font.name = "Arial"
    r.font.size = Pt(12)
    r.bold = bold
    return p


def body_para(doc, text):
    p = doc.add_paragraph()
    r = p.add_run(text)
    r.font.name = "Calibri"
    r.font.size = Pt(11)
    return p


def margins(doc):
    section = doc.sections[0]
    section.top_margin = Cm(2)
    section.right_margin = Cm(2)
    section.bottom_margin = Cm(2)
    section.left_margin = Cm(2)


def build_main():
    doc = docx.Document()
    margins(doc)
    cover_title(doc)
    cover_line(doc, "Nama Mahasiswa Contoh")
    last = cover_line(doc, "UNIVERSITAS CONTOH", bold=True)
    last.add_run().add_break(WD_BREAK.PAGE)
    body_para(doc, BODY_TEXT_1)
    cap = doc.add_paragraph(style="Caption")
    cap.alignment = WD_ALIGN_PARAGRAPH.CENTER
    r = cap.add_run("Gambar 1.1 Diagram alir penelitian")
    r.font.name = "Calibri"
    r.font.size = Pt(10)
    r.italic = True
    cap2 = doc.add_paragraph()
    r = cap2.add_run("Gambar 2.1 Contoh diagram")
    r.font.name = "Calibri"
    r.font.size = Pt(10)
    r.italic = True
    body_para(doc, BODY_TEXT_2)
    doc.save("sampul.docx")
    print("tulis sampul.docx")


def build_pbb():
    doc = docx.Document()
    margins(doc)
    cover_title(doc)
    cover_line(doc, "UNIVERSITAS CONTOH", bold=True)
    p = body_para(doc, BODY_TEXT_1)
    p.paragraph_format.page_break_before = True
    doc.save("sampul_pbb.docx")
    print("tulis sampul_pbb.docx")


def build_sect():
    doc = docx.Document()
    margins(doc)
    cover_title(doc)
    cover_line(doc, "UNIVERSITAS CONTOH", bold=True)
    doc.add_section()
    body_para(doc, BODY_TEXT_1)
    doc.save("sampul_sect.docx")
    print("tulis sampul_sect.docx")


if __name__ == "__main__":
    build_main()
    build_pbb()
    build_sect()
