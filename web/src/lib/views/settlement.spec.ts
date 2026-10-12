import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import { byEdge, status } from './settlement';

const edge = (child_id: string, outstanding: string, carried: string): Schemas['Edge'] => ({
	child_id,
	parent_id: 'home',
	claims: '0',
	stipend: '0',
	due: '0',
	paid: '0',
	carried,
	outstanding,
	to_settle: outstanding,
	implied_stipend: '0'
});

describe('settlement view', () => {
	it('tells new differences from past ones being paid down', () => {
		expect(status(edge('a', '10.00', '0.00'), false)).toBe('open');
		expect(status(edge('a', '0.00', '-30.00'), true)).toBe('settled');
		expect(status(edge('a', '-10.00', '-30.00'), true)).toBe('paying-down');
		expect(status(edge('a', '-30.00', '-30.00'), true)).toBe('difference');
		expect(status(edge('a', '5.00', '0.00'), true)).toBe('difference');
	});

	it('groups by edge, people before businesses, months in order', () => {
		const parties = new Map<string, Schemas['Party']>([
			['studio', { id: 'studio', name: 'Studio', kind: 'business' }],
			['sam', { id: 'sam', name: 'Sam', kind: 'person' }]
		]);
		const month = (m: string): Schemas['Statement'] => ({
			month: m,
			closed: m < '2026-03',
			currency_id: 'usd',
			unpriced: [],
			edges: [edge('studio', '0', '0'), edge('sam', '1.00', '0')]
		});
		const h = byEdge(
			{ currency_id: 'usd', unpriced: [], months: [month('2026-03'), month('2026-02')] },
			parties
		);
		expect(h.map((e) => e.child?.name)).toEqual(['Sam', 'Studio']);
		expect(h[0].months.map((m) => [m.month, m.status])).toEqual([
			['2026-02', 'difference'],
			['2026-03', 'open']
		]);
	});
});
