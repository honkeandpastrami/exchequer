import { describe, expect, it } from 'vitest';
import { hashQuery, withParam } from './query';

describe('query', () => {
	it('reads the query inside the hash', () => {
		const url = new URL('https://host/prefix/?ignored=1#/transactions?q=rent&from=2026-01-01');
		expect(hashQuery(url).get('q')).toBe('rent');
		expect(hashQuery(url).get('ignored')).toBeNull();
		expect(hashQuery(new URL('https://host/#/transactions')).size).toBe(0);
	});

	it('sets and clears', () => {
		const q = new URLSearchParams('q=rent&from=2026-01-01');
		expect(withParam(q, 'q', '')).toBe('from=2026-01-01');
		expect(withParam(q, 'to', '2026-02-01')).toBe('q=rent&from=2026-01-01&to=2026-02-01');
	});
});
