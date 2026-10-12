import { describe, expect, it } from 'vitest';
import { thisMonth } from '../dates';
import { readWindow } from './window';

describe('month window', () => {
	it('defaults to the last 12 months', () => {
		const w = readWindow(new URLSearchParams());
		expect(w.end).toBe(thisMonth());
		expect(w.span).toBe(12);
	});

	it('reads the URL, never past this month', () => {
		const w = readWindow(new URLSearchParams('end=2025-03&span=6'));
		expect(w).toEqual({ end: '2025-03', span: 6, start: '2024-10' });
		expect(readWindow(new URLSearchParams('end=2999-01')).end).toBe(thisMonth());
		expect(readWindow(new URLSearchParams('span=nope')).span).toBe(12);
	});
});
