export interface CsvTable {
  headers: string[];
  rows: string[][];
}

/**
 * Reads the run of characters that follows an opening quote, up to the closing
 * one. A doubled quote inside is an escaped quote, not the end of the field.
 * `end` is the index of the closing quote, or of the end of input if the field
 * was never closed.
 */
function readQuotedField(text: string, start: number): { value: string; end: number } {
  let value = '';
  let index = start;
  while (index < text.length) {
    const character = text[index];
    if (character !== '"') {
      value += character;
      index += 1;
      continue;
    }
    if (text[index + 1] !== '"') return { value, end: index };
    value += '"';
    index += 2;
  }
  return { value, end: index };
}

function readRows(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';

  const endField = () => {
    row.push(field);
    field = '';
  };
  const endRow = () => {
    endField();
    rows.push(row);
    row = [];
  };

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (character === '"') {
      const quoted = readQuotedField(text, index + 1);
      field += quoted.value;
      index = quoted.end;
    } else if (character === ',') endField();
    else if (character === '\n') endRow();
    else if (character !== '\r') field += character;
  }

  if (field.length > 0 || row.length > 0) endRow();
  return rows;
}

/**
 * RFC 4180 style parsing: quoted fields may contain commas, newlines and
 * doubled quotes. Anything else would mangle real exported spreadsheets.
 */
export function parseCsv(text: string): CsvTable {
  const nonEmpty = readRows(text).filter((candidate) => candidate.some((cell) => cell.trim() !== ''));
  if (nonEmpty.length === 0) return { headers: [], rows: [] };
  const [headers, ...body] = nonEmpty;
  return { headers, rows: body };
}

/**
 * Reads the cells out of a server-rendered table. The HTML is parsed with an
 * inert document and only `textContent` is taken, so no markup is carried over.
 */
export function tableFromHtml(html: string): CsvTable | null {
  const table = new DOMParser().parseFromString(html, 'text/html').querySelector('table');
  if (!table) return null;

  const readCells = (tableRow: HTMLTableRowElement): string[] =>
    Array.from(tableRow.cells).map((cell) => cell.textContent?.trim() ?? '');

  const headRow = table.tHead?.rows[0] ?? table.rows[0];
  if (!headRow) return null;
  const headers = readCells(headRow);

  const bodyRows = Array.from(table.tBodies).flatMap((body) => Array.from(body.rows));
  const source = bodyRows.length > 0 ? bodyRows : Array.from(table.rows).slice(1);
  return { headers, rows: source.map(readCells) };
}
