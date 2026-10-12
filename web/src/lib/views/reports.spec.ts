import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import { balanceSeries, buckets, byCategory, commoditiesIn } from './reports';

const USD = 'usd';
const EUR = 'eur';
const row = (r: Partial<Schemas['Row']> & { amount: string }): Schemas['Row'] => ({
	commodity_id: USD,
	...r
});

describe('balance series', () => {
	const rows = [
		row({ bucket: '2026-01-01', account_id: 'a', amount: '100.00' }),
		row({ bucket: '2026-02-01', account_id: 'a', amount: '150.00' }),
		row({ bucket: '2026-02-01', account_id: 'b', amount: '-20.00' }),
		row({ bucket: '2026-02-01', account_id: 'a', amount: '7', commodity_id: EUR })
	];
	const names = new Map([
		['a', 'Checking'],
		['b', 'Card']
	]);

	it('fills gaps and adds a total', () => {
		const at = buckets(rows);
		expect(at).toEqual(['2026-01-01', '2026-02-01']);
		expect(balanceSeries(rows, 'account_id', USD, names, at)).toEqual([
			{ name: 'Card', values: ['0.00', '-20.00'] },
			{ name: 'Checking', values: ['100.00', '150.00'] },
			{ name: 'Total', values: ['100.00', '130.00'] }
		]);
		expect(balanceSeries(rows, 'account_id', EUR, names, at)).toEqual([
			{ name: 'Checking', values: ['0', '7'] }
		]);
	});

	it('lists commodities, the preferred one first', () => {
		expect(commoditiesIn(rows, EUR)).toEqual([EUR, USD]);
		expect(commoditiesIn(rows, 'gbp')).toEqual([USD, EUR]);
	});
});

describe('by category', () => {
	const rows = [
		row({ bucket: '2026-01-01', category_id: 'food', amount: '-40.00' }),
		row({ bucket: '2026-02-01', category_id: 'food', amount: '-35.50' }),
		row({ bucket: '2026-02-01', category_id: 'rent', amount: '-900.00' }),
		row({ bucket: '2026-02-01', category_id: null, amount: '-5.00' }),
		row({ bucket: '2026-02-01', category_id: 'food', amount: '-1', commodity_id: EUR })
	];

	it('ranks spending, positive, per month', () => {
		const r = byCategory(rows, USD, ['2026-01', '2026-02', '2026-03'], true);
		expect(r.categories.map((c) => [c.category, c.total])).toEqual([
			['rent', '900.00'],
			['food', '75.50'],
			[null, '5.00']
		]);
		expect(r.categories[1].monthly).toEqual(['40.00', '35.50', '0.00']);
		expect(r.monthly).toEqual(['40.00', '940.50', '0.00']);
		expect(r.total).toBe('980.50');
	});

	it('keeps the sign for income', () => {
		const r = byCategory(rows, USD, ['2026-02'], false);
		expect(r.total).toBe('-940.50');
	});
});
