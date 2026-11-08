import { describe, expect, it } from 'vitest';
import {
  formatWon,
  nextRequestStates,
  parseImportJson,
  splitIdentifiers,
  validatePolicy,
  validateQuery,
} from '../src/application/AppsWebSrcApplicationRayportSession';
import {
  martExample,
  discoveryExample,
} from '../src/application/AppsWebSrcApplicationRayportExamples';
import type { Tool } from '../src/domain/AppsWebSrcDomainRayportTypes';

const tool = { max_days: 31 } as Tool;
describe('query conditions and data integrity', () => {
  it('handles leap days and rejects invalid calendar dates', () => {
    expect(
      validateQuery(
        { start_date: '2024-02-29', end_date_exclusive: '2024-03-01', store_ids: [] },
        tool,
      ),
    ).toBeNull();
    expect(
      validateQuery(
        { start_date: '2025-02-29', end_date_exclusive: '2025-03-01', store_ids: [] },
        tool,
      ),
    ).not.toBeNull();
    expect(
      validateQuery(
        { start_date: '2026-2-01', end_date_exclusive: '2026-02-02', store_ids: [] },
        tool,
      ),
    ).not.toBeNull();
  });
  it('rejects empty, reversed, and excessive ranges', () => {
    for (const end of ['2026-01-01', '2025-12-31', '2026-03-01'])
      expect(
        validateQuery({ start_date: '2026-01-01', end_date_exclusive: end, store_ids: [] }, tool),
      ).not.toBeNull();
    expect(
      validateQuery(
        { start_date: '2026-01-01', end_date_exclusive: '2026-02-01', store_ids: [] },
        tool,
      ),
    ).toBeNull();
  });
  it('normalizes but never invents store entitlement', () => {
    expect(splitIdentifiers(' S01, S02, S01, ,')).toEqual(['S01', 'S02']);
    expect(splitIdentifiers('')).toEqual([]);
  });
  it('does not show imprecise numeric values as money', () => {
    expect(formatWon(100000)).toBe('100,000원');
    expect(formatWon(Number.MAX_SAFE_INTEGER + 1)).toBe('표시 범위 초과');
    expect(formatWon(1.5)).toBe('표시 범위 초과');
  });
  it('rejects malformed, null, and oversized import documents', () => {
    for (const input of [
      '{',
      'null',
      '[]',
      '1',
      '"text"',
      JSON.stringify({ huge: 'x'.repeat(2_000_001) }),
    ])
      expect(() => parseImportJson(input)).toThrow();
    expect(parseImportJson('{"rows":[]}')).toEqual({ rows: [] });
  });
  it('validates budget integers', () => {
    expect(
      validatePolicy({
        paused: false,
        per_user_per_minute: 1,
        source_per_minute: 20,
        max_concurrent: 3,
      }),
    ).toBeNull();
    expect(
      validatePolicy({
        paused: false,
        per_user_per_minute: 0,
        source_per_minute: 20,
        max_concurrent: 3,
      }),
    ).not.toBeNull();
  });
  it('separates publication from requester-confirmed resolution', () => {
    expect(nextRequestStates('ready', false, true)).toEqual(['resolved']);
    expect(nextRequestStates('submitted', false, true)).toEqual([]);
    expect(nextRequestStates('resolved', true, true)).toEqual([]);
  });
  it('does not let an administrator resolve another requester’s question or skip implementation', () => {
    expect(nextRequestStates('ready', true, false)).toEqual(['building']);
    expect(nextRequestStates('in_review', true, false)).not.toContain('ready');
    expect(nextRequestStates('ready', false, false)).toEqual([]);
  });
  it('creates explicit synthetic examples with a complete snapshot date', () => {
    const example = martExample(new Date('2026-11-08T09:00:00Z'));
    expect(example.complete_through).toBe('2026-11-07');
    expect(example.rows).toHaveLength(2);
    expect(example.rows.reduce((total, row) => total + row.gross_minor - row.cancel_minor, 0)).toBe(
      175000,
    );
    expect(discoveryExample().observations[0].sql_text).toContain(':end_date_exclusive');
    expect(discoveryExample().observations[0].sql_text).toContain(':store_id');
  });
});
