import { useCallback, useMemo, useState } from 'react';
import { fetchRaw } from '../../api/client';
import { useAsyncResource } from '../../hooks/useAsyncResource';
import { parseCsv, tableFromHtml, type CsvTable } from '../../lib/csv';
import { ErrorState, LoadingState } from '../ui/States';
import type { ViewerProps } from './viewer-types';

type SortDirection = 'asc' | 'desc';

function compareCells(left: string, right: string): number {
  const leftNumber = Number(left.replace(/[,\s]/g, ''));
  const rightNumber = Number(right.replace(/[,\s]/g, ''));
  const bothNumeric =
    left.trim() !== '' && right.trim() !== '' && Number.isFinite(leftNumber) && Number.isFinite(rightNumber);
  if (bothNumeric) return leftNumber - rightNumber;
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: 'base' });
}

function SortableTable({ table }: { table: CsvTable }) {
  const [sort, setSort] = useState<{ column: number; direction: SortDirection } | null>(null);

  const rows = useMemo(() => {
    if (!sort) return table.rows;
    const sorted = [...table.rows];
    sorted.sort((a, b) => {
      const result = compareCells(a[sort.column] ?? '', b[sort.column] ?? '');
      return sort.direction === 'asc' ? result : -result;
    });
    return sorted;
  }, [table.rows, sort]);

  const toggle = (column: number) =>
    setSort((current) =>
      current?.column === column
        ? { column, direction: current.direction === 'asc' ? 'desc' : 'asc' }
        : { column, direction: 'asc' },
    );

  return (
    <div className="doc__inner">
      <div className="csv-wrap" tabIndex={0} role="region" aria-label="Spreadsheet contents">
        <table className="csv-table">
          <thead>
            <tr>
              {table.headers.map((header, column) => {
                const active = sort?.column === column;
                return (
                  <th
                    key={`${header}-${column}`}
                    scope="col"
                    aria-sort={active ? (sort.direction === 'asc' ? 'ascending' : 'descending') : 'none'}
                  >
                    <button type="button" className="csv-table__sort" onClick={() => toggle(column)}>
                      {header || `Column ${column + 1}`}
                      <span aria-hidden="true">{active ? (sort.direction === 'asc' ? '▲' : '▼') : '↕'}</span>
                      <span className="sr-only">
                        {active ? `sorted ${sort.direction === 'asc' ? 'ascending' : 'descending'}` : ', sort by this column'}
                      </span>
                    </button>
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, index) => (
              <tr key={index}>
                {table.headers.map((_, column) => (
                  <td key={column}>{row[column] ?? ''}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="csv-count">
        {rows.length} {rows.length === 1 ? 'row' : 'rows'} · {table.headers.length} columns
      </p>
    </div>
  );
}

/**
 * The server normally sends the sheet pre-rendered as an HTML table; when it
 * does not, the raw file is fetched and parsed here instead.
 */
export function CsvViewer({ payload, rootId }: ViewerProps) {
  const fromHtml = useMemo(() => (payload.html ? tableFromHtml(payload.html) : null), [payload.html]);
  const path = payload.meta.path;

  const load = useCallback(
    async (signal: AbortSignal): Promise<CsvTable | null> => {
      if (fromHtml) return fromHtml;
      return parseCsv(await fetchRaw(rootId, path, signal));
    },
    [fromHtml, rootId, path],
  );
  const resource = useAsyncResource(load);

  if (resource.error) return <ErrorState error={resource.error} onRetry={resource.reload} />;
  if (!resource.data) return resource.loading ? <LoadingState label="Loading spreadsheet…" /> : null;
  return <SortableTable table={resource.data} />;
}
