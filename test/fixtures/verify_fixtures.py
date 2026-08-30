#!/usr/bin/env python3
"""Verify the kbviewer test fixtures are structurally what the tests assume.

Run from anywhere:  python3 test/fixtures/verify_fixtures.py
Exits non-zero if any check fails.
"""
from __future__ import annotations

import csv
import json
import re
import struct
import sys
import xml.dom.minidom
import zipfile
import zlib
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent
VAULT = FIXTURES / "obsidian-vault"
PLAIN = FIXTURES / "plain-markdown"
MEDIA = FIXTURES / "mixed-media"

DOCX_REQUIRED_PARTS = (
    "[Content_Types].xml",
    "_rels/.rels",
    "word/document.xml",
    "word/_rels/document.xml.rels",
)
EXPECTED_CSV_RECORDS = 12
EXPECTED_CSV_COLUMNS = 5

results: list[tuple[bool, str]] = []


def check(condition: bool, message: str) -> bool:
    results.append((bool(condition), message))
    return bool(condition)


def passed(message: str) -> None:
    """Record a check that succeeded. Reads better than `check(True, ...)`."""
    results.append((True, message))


def failed(message: str) -> None:
    """Record a check that failed, typically from an exception handler."""
    results.append((False, message))


def check_pdf(path: Path) -> None:
    pdf = path.read_bytes()
    check(pdf.startswith(b"%PDF-"), f"{path.name}: starts with %PDF- header ({pdf[:8]!r})")
    check(pdf.rstrip().endswith(b"%%EOF"), f"{path.name}: ends with %%EOF trailer")
    check(b"/Type /Catalog" in pdf, f"{path.name}: has a document catalog")
    check(b"/Type /Page" in pdf, f"{path.name}: has at least one page object")

    match = re.search(rb"startxref\s+(\d+)", pdf)
    if check(match is not None, f"{path.name}: has a startxref pointer"):
        offset = int(match.group(1))
        check(pdf[offset:offset + 4] == b"xref",
              f"{path.name}: startxref offset {offset} lands on the xref table")


def check_docx(path: Path) -> None:
    if not check(zipfile.is_zipfile(path), f"{path.name}: is a zip archive"):
        return
    with zipfile.ZipFile(path) as archive:
        check(archive.testzip() is None, f"{path.name}: every zip member's CRC checks out")
        names = set(archive.namelist())
        for part in DOCX_REQUIRED_PARTS:
            check(part in names, f"{path.name}: contains required part {part}")
        for part in sorted(n for n in names if n.endswith((".xml", ".rels"))):
            try:
                xml.dom.minidom.parseString(archive.read(part))
                ok = True
            except Exception as exc:                      # noqa: BLE001 - report, do not raise
                ok = False
                part = f"{part} ({exc})"
            check(ok, f"{path.name}: {part} is well-formed XML")
        body = archive.read("word/document.xml").decode("utf-8")
        check("<w:body>" in body, f"{path.name}: document.xml has a <w:body>")
        check('w:val="Heading1"' in body, f"{path.name}: document.xml uses a heading style")
        check("<w:numPr>" in body, f"{path.name}: document.xml contains a list")
        check("<w:tbl>" in body, f"{path.name}: document.xml contains a table")


def check_png(path: Path) -> None:
    png = path.read_bytes()
    if not check(png[:8] == b"\x89PNG\r\n\x1a\n", f"{path.name}: PNG magic bytes"):
        return
    check(png[-8:-4] == b"IEND", f"{path.name}: ends with an IEND chunk")

    offset, seen, idat = 8, [], b""
    while offset < len(png):
        length = struct.unpack(">I", png[offset:offset + 4])[0]
        tag = png[offset + 4:offset + 8]
        payload = png[offset + 8:offset + 8 + length]
        stored_crc = struct.unpack(">I", png[offset + 8 + length:offset + 12 + length])[0]
        if zlib.crc32(tag + payload) & 0xFFFFFFFF != stored_crc:
            failed(f"{path.name}: chunk {tag.decode()} CRC mismatch")
            return
        seen.append(tag)
        if tag == b"IDAT":
            idat += payload
        offset += 12 + length
    check(seen[0] == b"IHDR", f"{path.name}: first chunk is IHDR, all chunk CRCs valid")
    width, height, depth, colour = struct.unpack(">IIBB", png[16:26])
    try:
        raw = zlib.decompress(idat)
        pixels_ok = len(raw) == height * (1 + width * 3)
    except zlib.error:
        pixels_ok = False
    check(pixels_ok, f"{path.name}: IDAT inflates to {width}x{height} truecolour "
                     f"({depth}-bit, colour type {colour})")


def check_jpeg(path: Path) -> None:
    jpeg = path.read_bytes()
    check(jpeg[:2] == b"\xff\xd8", f"{path.name}: JPEG SOI marker")
    check(jpeg[-2:] == b"\xff\xd9", f"{path.name}: JPEG EOI marker")
    check(jpeg[6:10] == b"JFIF", f"{path.name}: JFIF APP0 segment")

    offset, markers = 2, []
    while offset < len(jpeg) - 1 and jpeg[offset] == 0xFF:
        marker = jpeg[offset + 1]
        markers.append(marker)
        if marker == 0xDA:
            break
        length = struct.unpack(">H", jpeg[offset + 2:offset + 4])[0]
        offset += 2 + length
    check(0xC0 in markers, f"{path.name}: has a baseline SOF0 frame header")
    check(markers.count(0xC4) >= 2, f"{path.name}: has DC and AC Huffman tables")
    check(0xDB in markers, f"{path.name}: has a quantisation table")
    check(0xDA in markers, f"{path.name}: has a start-of-scan marker")
    if 0xC0 in markers:
        sof = jpeg.index(b"\xff\xc0")
        height, width = struct.unpack(">HH", jpeg[sof + 5:sof + 9])
        check(width > 0 and height > 0, f"{path.name}: frame header declares {width}x{height}")


def check_svg(path: Path) -> None:
    try:
        document = xml.dom.minidom.parse(str(path))
        root = document.documentElement.tagName
    except Exception as exc:                              # noqa: BLE001 - report, do not raise
        failed(f"{path.name}: parses as XML ({exc})")
        return
    check(root == "svg", f"{path.name}: parses as XML, root element is <{root}>")
    check(document.documentElement.getAttribute("xmlns") == "http://www.w3.org/2000/svg",
          f"{path.name}: declares the SVG namespace")


def check_csv(path: Path) -> None:
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.reader(handle))
    check(len(rows) == EXPECTED_CSV_RECORDS,
          f"{path.name}: parses to {len(rows)} records (expected {EXPECTED_CSV_RECORDS})")
    widths = {len(row) for row in rows}
    check(widths == {EXPECTED_CSV_COLUMNS},
          f"{path.name}: every record has {EXPECTED_CSV_COLUMNS} columns (saw {sorted(widths)})")
    check(any("," in field for row in rows for field in row),
          f"{path.name}: contains a quoted field with an embedded comma")
    check(any("\n" in field for row in rows for field in row),
          f"{path.name}: contains a quoted field with an embedded newline")
    check(len(path.read_text(encoding="utf-8").splitlines()) > len(rows),
          f"{path.name}: has more physical lines than records, so the newline really is embedded")


def check_json(path: Path) -> None:
    try:
        json.loads(path.read_text(encoding="utf-8"))
        passed(f"{path.relative_to(FIXTURES)}: parses as JSON")
    except json.JSONDecodeError as exc:
        failed(f"{path.relative_to(FIXTURES)}: parses as JSON ({exc})")


def check_vault_structure() -> None:
    check((VAULT / ".obsidian" / "appearance.json").is_file(),
          "obsidian-vault: .obsidian/appearance.json exists (triggers vault mode)")
    accent = json.loads((VAULT / ".obsidian" / "appearance.json").read_text())["accentColor"]
    check(re.fullmatch(r"#[0-9a-fA-F]{6}", accent) is not None,
          f"obsidian-vault: appearance.json accentColor is a hex colour ({accent})")
    check((VAULT / "index.md").is_file(), "obsidian-vault: index.md landing page exists")

    duplicates = sorted(p.relative_to(VAULT).as_posix() for p in VAULT.rglob("Meeting Notes.md"))
    check(len(duplicates) == 2, f"obsidian-vault: ambiguous basename present ({duplicates})")
    if len(duplicates) == 2:
        shortest = min(duplicates, key=lambda p: (p.count("/"), len(p)))
        check(shortest == "meetings/Meeting Notes.md",
              f"obsidian-vault: shortest-path winner is {shortest}")

    check(any("知识管理" in p.name for p in VAULT.rglob("*.md")),
          "obsidian-vault: a note with a CJK filename exists")
    check(any(" " in p.name for p in VAULT.rglob("*.md")),
          "obsidian-vault: a note with spaces in its filename exists")

    depth = max(len(p.relative_to(VAULT).parts) for p in VAULT.rglob("*.md"))
    check(depth >= 4, f"obsidian-vault: deepest note is {depth} path segments (needs >= 4)")

    text = "\n".join(p.read_text(encoding="utf-8") for p in VAULT.rglob("*.md"))
    for label, pattern in (
        ("plain [[Note]]", r"\[\[Glossary\]\]"),
        ("aliased [[Note|alias]]", r"\[\[[^\]|#]+\|[^\]]+\]\]"),
        ("heading [[Note#Heading]]", r"\[\[[^\]#]+#[^\]]+\]\]"),
        ("local [[#Heading]]", r"\[\[#[^\]]+\]\]"),
        ("unresolved [[Does Not Exist]]", r"\[\[Does Not Exist\]\]"),
        ("image embed", r"!\[\[image\.png\]\]"),
        ("pdf embed", r"!\[\[document\.pdf\]\]"),
        ("note transclusion", r"!\[\[Glossary\]\]"),
        ("callout, untitled", r">\s*\[!note\]"),
        ("callout, titled", r">\s*\[!warning\] With A Title"),
        ("callout, tip", r">\s*\[!tip\]"),
        ("nested callout", r">\s*>\s*\[!"),
        ("inline math", r"\$x\^2\$"),
        ("display math", r"(?m)^\$\$$"),
        ("mermaid fence", r"```mermaid"),
        ("rust fence", r"```rust"),
        ("python fence", r"```python"),
        ("json fence", r"```json"),
        ("GFM table", r"(?m)^\|\s*---"),
        ("task list", r"(?m)^\s*- \[x\] "),
        ("strikethrough", r"~~[^~]+~~"),
        ("footnote definition", r"(?m)^\[\^\w+\]:"),
        ("tag inside a code fence", r"#definitely-not-a-tag"),
        ("hex colour in a heading", r"(?m)^#{2,6} .*#ffffff"),
    ):
        check(re.search(pattern, text) is not None, f"obsidian-vault: has {label}")

    for note in VAULT.rglob("*.md"):
        opens_with_delimiter = note.read_text(encoding="utf-8").startswith("---\n")
        check(opens_with_delimiter,
              f"obsidian-vault: {note.relative_to(VAULT)} opens with frontmatter")
    for key in ("title:", "tags:", "date:", "aliases:"):
        holders = [p for p in VAULT.rglob("*.md") if key in p.read_text(encoding="utf-8")[:400]]
        check(len(holders) >= 3, f"obsidian-vault: '{key.rstrip(':')}' frontmatter used by "
                                 f"{len(holders)} notes")


def check_plain_structure() -> None:
    check(not any(PLAIN.rglob(".obsidian")),
          "plain-markdown: no .obsidian directory anywhere (proves non-vault mode)")
    check((PLAIN / "README.md").is_file() and not (PLAIN / "index.md").exists(),
          "plain-markdown: README.md at root and no index.md, exercising the landing-page fallback")
    check((PLAIN / "guides" / "index.md").is_file(),
          "plain-markdown: guides/ has an index.md")
    check(not (PLAIN / "reference" / "index.md").exists()
          and not (PLAIN / "reference" / "README.md").exists()
          and any((PLAIN / "reference").glob("*.md")),
          "plain-markdown: reference/ has no index file, forcing a generated listing")

    text = "\n".join(p.read_text(encoding="utf-8") for p in PLAIN.rglob("*.md"))
    check("[[" not in text, "plain-markdown: contains no wikilink syntax at all")
    for label, pattern in (
        ("./sibling.md links", r"\]\(\./[a-z-]+\.md\)"),
        ("../parent.md links", r"\]\(\.\./[a-z-]+\.md\)"),
        ("links into a subfolder", r"\]\(\./guides/[a-z-]+\.md\)"),
        ("a link with a fragment", r"\]\([^)]+\.md#[^)]+\)"),
        ("relative image references", r"!\[[^\]]*\]\((\./|\.\./)?assets/[a-z-]+\.png\)"),
    ):
        check(re.search(pattern, text) is not None, f"plain-markdown: has {label}")

    broken = []
    for note in PLAIN.rglob("*.md"):
        body = note.read_text(encoding="utf-8")
        for match in re.finditer(r"\]\((?!https?:|#)([^)#]+)(?:#[^)]*)?\)", body):
            target = match.group(1)
            resolved = (note.parent / target).resolve()
            if not resolved.exists():
                broken.append(f"{note.relative_to(PLAIN)} -> {target}")
    expected_broken = {"README.md -> ./does-not-exist.md",
                       "reference/http-api.md -> ./nope.md"}
    check(set(broken) == expected_broken,
          f"plain-markdown: exactly the two deliberately broken links are broken "
          f"(found {sorted(broken)})")


def check_media_structure() -> None:
    check((MEDIA / "index.md").is_file(), "mixed-media: index.md landing page exists")
    check(not any(MEDIA.rglob(".obsidian")), "mixed-media: no .obsidian directory")
    unknown = MEDIA / "opaque.bin"
    check(unknown.is_file() and b"\x00" in unknown.read_bytes(),
          "mixed-media: opaque.bin exists and is binary (download-only fallback)")
    extensionless = MEDIA / "LICENSE"
    check(extensionless.is_file() and extensionless.suffix == "",
          "mixed-media: LICENSE has no extension (download-only fallback)")
    check((MEDIA / "notes.txt").is_file(), "mixed-media: notes.txt exists")


def main() -> int:
    check_vault_structure()
    check_plain_structure()
    check_media_structure()

    check_pdf(MEDIA / "report.pdf")
    check_pdf(VAULT / "attachments" / "document.pdf")
    check_docx(MEDIA / "meeting-minutes.docx")
    check_png(MEDIA / "logo.png")
    check_png(VAULT / "attachments" / "image.png")
    check_png(PLAIN / "assets" / "diagram.png")
    check_png(PLAIN / "assets" / "screenshot.png")
    check_jpeg(MEDIA / "photo.jpg")
    check_svg(MEDIA / "chart.svg")
    check_svg(VAULT / "attachments" / "diagram.svg")
    check_csv(MEDIA / "inventory.csv")
    check_json(MEDIA / "config.json")
    for name in ("appearance.json", "app.json", "core-plugins.json"):
        check_json(VAULT / ".obsidian" / name)

    failures = [message for ok, message in results if not ok]
    for ok, message in results:
        print(f"{'PASS' if ok else 'FAIL'}  {message}")
    print(f"\n{len(results) - len(failures)}/{len(results)} checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
