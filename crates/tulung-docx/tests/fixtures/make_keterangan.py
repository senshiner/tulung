"""Buat fixture keterangan.docx dan keterangan-teks.docx untuk tes
pembuatan daftar tabel/gambar Doc Format.

keterangan.docx: daftar isi basi (1 entri) + 1 Heading 1 + tabel 2x2
dengan caption SEQ "Tabel 1" + caption SEQ "Gambar 1", TANPA daftar
tabel/gambar. Perbaikan harus membuatkan kedua daftar itu sesudah
daftar isi. Helper field/caption disalin dari make_daftar.py.

keterangan-teks.docx: heading + keterangan yang diketik manual
("Tabel 1 ..." / "Gambar 1 ..." sebagai teks biasa, tanpa field SEQ).
Daftar tabel/gambar tidak bisa mengumpulkan keterangan tanpa field
SEQ (sama seperti Word), jadi perbaikan tidak boleh membuatkan
daftar untuk dokumen ini.

Jalankan: python3 make_keterangan.py -> menulis kedua .docx di
folder ini.
"""
import docx
from docx.enum.style import WD_STYLE_TYPE
from docx.oxml.ns import nsdecls
from docx.oxml import parse_xml

FILLER = "Paragraf isi untuk dokumen uji keterangan. "


def add_field_paragraphs(doc, instr, stale_entries, field_style=None):
    """Sisipkan field kompleks: paragraf begin+instr+separate, lalu
    satu paragraf per entri basi, lalu paragraf end."""
    p = doc.add_paragraph()
    if field_style is not None:
        p.style = field_style
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
    """Paragraf caption berfield SEQ, bentuk yang sama dengan
    make_daftar.py (bookmark Label/Number di sekeliling field)."""
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


def build_keterangan():
    doc = docx.Document()
    toc1 = doc.styles.add_style("TOC 1", WD_STYLE_TYPE.PARAGRAPH)
    doc.styles.add_style("Table of Figures", WD_STYLE_TYPE.PARAGRAPH)
    doc.add_paragraph("DAFTAR ISI")
    add_field_paragraphs(
        doc, 'TOC \\o "1-3" \\h \\z \\u',
        [("ENTRI BASI\t99", toc1)],
    )
    doc.add_heading("Bab Satu: Pendahuluan", level=1)
    for _ in range(2):
        doc.add_paragraph(FILLER * 2)
    table = doc.add_table(rows=2, cols=2)
    for row in table.rows:
        for cell in row.cells:
            cell.text = "Sel tabel."
    add_caption(doc, "Tabel", "Contoh data", 1)
    doc.add_paragraph(FILLER * 2)
    add_caption(doc, "Gambar", "Contoh gambar", 4)
    doc.add_paragraph(FILLER * 2)
    doc.save("keterangan.docx")
    print("tulis keterangan.docx")


def build_keterangan_teks():
    doc = docx.Document()
    doc.add_heading("Bab Satu: Pendahuluan", level=1)
    doc.add_paragraph(FILLER * 2)
    doc.add_paragraph("Tabel 1 Contoh data")
    doc.add_paragraph(FILLER * 2)
    doc.add_paragraph("Gambar 1 Contoh gambar")
    doc.add_paragraph(FILLER * 2)
    doc.save("keterangan-teks.docx")
    print("tulis keterangan-teks.docx")


if __name__ == "__main__":
    build_keterangan()
    build_keterangan_teks()
