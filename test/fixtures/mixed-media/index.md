# Mixed Media

A root with almost no Markdown in it. Its job is to prove kbview works as a
*document* browser, not only as a Markdown renderer. There is no `.obsidian/`
directory here, and no wikilinks — this file is the only Markdown in the folder,
and it exists so the root has a landing page.

## What is here, and why

| File | Must do |
| --- | --- |
| `report.pdf` | Render inline in the PDF viewer. Valid header, xref table and `%%EOF`. |
| `meeting-minutes.docx` | Convert to HTML: title, two heading levels, bold/italic/strikethrough/underline runs, a bulleted list with a nested level, a numbered list, a 3-row table, a block quote and an external hyperlink. |
| `logo.png` | Render as an image. Real 32x32 truecolour PNG. |
| `photo.jpg` | Render as an image. Real baseline JPEG, 16x16, 4:4:4. |
| `chart.svg` | Render as an image, and be served with the right content type. |
| `inventory.csv` | Render as a table: 12 records, 5 columns, including a field with a quoted comma and a field with a quoted newline. |
| `notes.txt` | Render as preformatted text, **not** through the Markdown parser. |
| `config.json` | Render as text (ideally syntax-highlighted), with non-ASCII intact. |
| `opaque.bin` | Fall back to download-only: unknown extension, binary content. |
| `LICENSE` | Fall back to download-only: no extension at all. |

## Things to click through by hand

1. Open `inventory.csv` — row 11 must stay a single record even though it
   contains a line break.
2. Open `opaque.bin` — you should get a download prompt, not a wall of mojibake.
3. Open `LICENSE` — an extensionless file must not crash the type sniffer.
4. Open `meeting-minutes.docx` — the table's header row should be distinguishable
   from the body rows.
