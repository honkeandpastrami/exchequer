import { describe, expect, it } from 'vitest';
import type { Reference } from '../reference';
import { chips } from './chips';

const ref: Reference = {
	parties: new Map([['sam', { id: 'sam', name: 'Sam', kind: 'person' }]]),
	commodities: new Map([['usd', { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 }]]),
	accounts: new Map(),
	counterparties: new Map(),
	categories: new Map([['food', { id: 'food', name: 'Food' }]]),
	tags: new Map()
};

describe('chips', () => {
	it('names every filter value, links included', () => {
		const labels = chips(
			{
				q: 'rent',
				from: '2026-06-01',
				purpose_id: ['sam', 'none'],
				kind: ['expense'],
				category_id: ['none', 'food'],
				commodity_id: ['usd'],
				settles: '2026-06',
				party_id: 'sam',
				line: 'claim',
				unpriced: true
			},
			ref
		).map((c) => c.label);
		expect(labels).toEqual([
			'“rent”',
			'From Jun 1, 2026',
			'For Sam',
			'No purpose',
			'Expense',
			'Uncategorized',
			'Food',
			'USD',
			'Sam’s settlement for Jun 2026, claims',
			'No value in USD'
		]);
	});

	it('says which value each chip removes', () => {
		expect(chips({ kind: ['income', 'expense'] }, ref)).toEqual([
			{ key: 'kind', value: 'income', label: 'Income' },
			{ key: 'kind', value: 'expense', label: 'Expense' }
		]);
	});
});
