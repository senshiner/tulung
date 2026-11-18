"""Buat fixture daftar.docx untuk tes regenerasi daftar Doc Format.

Isi: daftar isi basi (2 entri, style TOC1/TOC2 seperti hasil Word),
3 Heading 1 + 1 Heading 2, tabel 2x2 + caption SEQ "Tabel 1",
daftar tabel basi (style TableofFigures), caption SEQ "Gambar 1",
daftar gambar basi, kutipan + daftar pustaka basi (style Bibliography).

Entri basi memakai style daftar seperti yang ditulis Word, supaya
banding teks (docx_text) tidak menghitung entri daftar sebagai isi.

Bagian sumber pustaka (customXml/item1.xml) disalin dari dokumen
probe /tmp/toc-research/pustaka.docx (sumber "Buku1", Winoto 2024):
disisipkan lewat bedah zip setelah python-docx menyimpan dokumen.

Jalankan: python3 make_daftar.py  -> menulis daftar.docx di folder ini.
"""
import shutil
import zipfile

import docx
from docx.oxml.ns import nsdecls
from docx.oxml import parse_xml

FILLER = "Paragraf isi untuk dokumen uji daftar. "

SRC_DOCX = "/tmp/toc-research/pustaka.docx"
OUT = "daftar.docx"


def add_field_paragraphs(doc, instr, stale_entries, field_style=None):
    """Sisipkan field kompleks: paragraf begin+instr+separate, lalu
    satu paragraf per entri basi, lalu paragraf end. Paragraf field
    boleh diberi style (daftar tabel hasil insert_table_of_figures
    memakai style TableofFigures + tab kanan)."""
    p = doc.add_paragraph()
    if field_style is not None:
        p.style = field_style
        pPr = p._p.get_or_add_pPr()
        tabs = parse_xml(
            f'<w:tabs {nsdecls("w")}><w:tab w:val="right" w:pos="9360" w:leader="dot"/></w:tabs>'
        )
        pPr.append(tabs)
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="begin"/>'))
    r = p.add_run()
    r._r.append(parse_xml(
        f'<w:instrText {nsdecls("w")} xml:space="preserve"> {instr} </w:instrText>'
    ))
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="separate"/>'))
    for text, style in stale_entries:
        entry = doc.add_paragraph(text)
        entry.style = style
    p = doc.add_paragraph()
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="end"/>'))


def add_caption(doc, label, text, base_id):
    """Paragraf caption seperti hasil insert_caption rdocx: bookmark
    Label di sekeliling 'Tabel ', bookmark Number di sekeliling field
    SEQ (tanpa hasil cache), bookmark keseluruhan caption.
    base_id: id bookmark awal (harus unik di dokumen)."""
    p = doc.add_paragraph()
    pe = p._p
    pe.append(parse_xml(
        f'<w:bookmarkStart {nsdecls("w")} w:id="{base_id}" w:name="{label}1"/>'
    ))
    pe.append(parse_xml(
        f'<w:bookmarkStart {nsdecls("w")} w:id="{base_id + 1}" w:name="{label}1_Label"/>'
    ))
    p.add_run(f"{label} ")
    pe.append(parse_xml(
        f'<w:bookmarkStart {nsdecls("w")} w:id="{base_id + 2}" w:name="{label}1_Number"/>'
    ))
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="begin"/>'))
    r = p.add_run()
    r._r.append(parse_xml(
        f'<w:instrText {nsdecls("w")} xml:space="preserve"> SEQ {label} </w:instrText>'
    ))
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="separate"/>'))
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="end"/>'))
    pe.append(parse_xml(
        f'<w:bookmarkEnd {nsdecls("w")} w:id="{base_id + 2}"/>'
    ))
    pe.append(parse_xml(
        f'<w:bookmarkEnd {nsdecls("w")} w:id="{base_id + 1}"/>'
    ))
    p.add_run(f" {text}")
    pe.append(parse_xml(
        f'<w:bookmarkEnd {nsdecls("w")} w:id="{base_id}"/>'
    ))
    return p


def build():
    doc = docx.Document()
    # Template python-docx tidak punya style daftar; buat seperti Word.
    from docx.enum.style import WD_STYLE_TYPE
    toc1 = doc.styles.add_style("TOC 1", WD_STYLE_TYPE.PARAGRAPH)
    toc2 = doc.styles.add_style("TOC 2", WD_STYLE_TYPE.PARAGRAPH)
    tof = doc.styles.add_style("Table of Figures", WD_STYLE_TYPE.PARAGRAPH)
    bib = doc.styles.add_style("Bibliography", WD_STYLE_TYPE.PARAGRAPH)
    doc.add_paragraph("DAFTAR ISI")
    add_field_paragraphs(
        doc, 'TOC \\o "1-3" \\h \\z \\u',
        [("ENTRI BASI SATU\t99", toc1), ("ENTRI BASI DUA\t99", toc2)],
    )
    doc.add_heading("Bab Satu: Pendahuluan", level=1)
    for _ in range(3):
        doc.add_paragraph(FILLER * 2)
    doc.add_heading("Sub Satu: Latar Belakang", level=2)
    for _ in range(2):
        doc.add_paragraph(FILLER * 2)
    doc.add_heading("Bab Dua: Penutup", level=1)
    for _ in range(2):
        doc.add_paragraph(FILLER * 2)

    table = doc.add_table(rows=2, cols=2)
    for row in table.rows:
        for cell in row.cells:
            cell.text = "Sel tabel."
    add_caption(doc, "Tabel", "Contoh data", 1)
    doc.add_paragraph("DAFTAR TABEL")
    add_field_paragraphs(
        doc, 'TOC \\c Tabel \\p "\\t" \\h',
        [("Tabel 1 Contoh data\t99", tof)],
        field_style=tof,
    )
    add_caption(doc, "Gambar", "Contoh gambar", 4)
    doc.add_paragraph("DAFTAR GAMBAR")
    add_field_paragraphs(
        doc, 'TOC \\c Gambar \\p "\\t" \\h',
        [("Gambar 1 Contoh gambar\t99", tof)],
        field_style=tof,
    )

    doc.add_heading("Kutipan", level=1)
    p = doc.add_paragraph("Lihat pendapat Winoto ")
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="begin"/>'))
    r = p.add_run()
    r._r.append(parse_xml(
        f'<w:instrText {nsdecls("w")} xml:space="preserve"> CITATION Buku1 \\l 1033 </w:instrText>'
    ))
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="separate"/>'))
    p.add_run("(Winoto, 2024)")
    r = p.add_run()
    r._r.append(parse_xml(f'<w:fldChar {nsdecls("w")} w:fldCharType="end"/>'))
    p.add_run(".")
    doc.add_paragraph("DAFTAR PUSTAKA")
    add_field_paragraphs(
        doc, 'BIBLIOGRAPHY \\l 1033',
        [("ENTRI PUSTAKA BASI", bib)],
    )
    doc.save(OUT)
    print("tulis", OUT)


def inject_bibliography_sources():
    """Template python-docx sudah berisi customXml/item1.xml KOSONG
    (elemen Sources tanpa Source) lengkap dengan relasi dan
    content-type. Jadi cukup ganti isi file itu dengan sumber pustaka
    dari pustaka.docx; tidak ada yang perlu ditambah."""
    src = zipfile.ZipFile(SRC_DOCX)
    sources = src.read("customXml/item1.xml")
    src.close()
    tmp = OUT + ".tmp"
    zin = zipfile.ZipFile(OUT)
    zout = zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED)
    for item in zin.infolist():
        data = zin.read(item.filename)
        if item.filename == "customXml/item1.xml":
            data = sources
        zout.writestr(item, data)
    zin.close()
    zout.close()
    shutil.move(tmp, OUT)
    print("suntik sumber pustaka selesai")


if __name__ == "__main__":
    build()
    inject_bibliography_sources()
