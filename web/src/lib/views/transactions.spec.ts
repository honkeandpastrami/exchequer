import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import type { Reference } from '../reference';
import { categoryNames, effectOn, legName, quantities, runningBalances } from './transactions';

const usd = { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 } as const;
const acct = (id: string): Schemas['Account'] => ({
	id,
	name: id,
	owner_id: 'sam',
	kind: 'checking',
	liquid: true,
	archived: false,
	numbers: []
});
const ref: Reference = {
	parties: new Map([['sam', { id: 'sam', name: 'Sam', kind: 'person' }]]),
	commodities: new Map([['usd', usd]]),
	accounts: new Map([
		['checking', acct('checking')],
		['savings', acct('savings')]
	]),
	counterparties: new Map([['grocer', { id: 'grocer', name: 'Grocer' }]]),
	categories: new Map([['food', { id: 'food', name: 'Food' }]]),
	tags: new Map()
};
const split = (
	from: string,
	to: string,
	quantity: string,
	category_id?: string
): Schemas['Split'] => ({
	from:
		from === 'grocer'
			? { counterparty_id: from, commodity_id: 'usd', quantity }
			: { account_id: from, commodity_id: 'usd', quantity },
	to:
		to === 'grocer'
			? { counterparty_id: to, commodity_id: 'usd', quantity }
			: { account_id: to, commodity_id: 'usd', quantity },
	kind: 'expense',
	category_id
});
const tx = (splits: Schemas['Split'][]): Schemas['Transaction'] => ({
	id: 't',
	date: '2026-03-01',
	description: 'x',
	tags: [],
	import_batch_id: 'b',
	external_refs: [],
	splits
});

describe('transactions view', () => {
	const t = tx([
		split('checking', 'grocer', '12.50', 'food'),
		split('checking', 'savings', '100.00')
	]);

	it('nets the effect on one account', () => {
		expect(effectOn(t, 'checking', ref)).toEqual([{ commodity: usd, quantity: '-112.50' }]);
		expect(effectOn(t, 'savings', ref)).toEqual([{ commodity: usd, quantity: '100.00' }]);
	});

	it('drops a commodity that nets to zero', () => {
		const roundTrip = tx([
			split('checking', 'savings', '5.00'),
			split('savings', 'checking', '5.00')
		]);
		expect(effectOn(roundTrip, 'checking', ref)).toEqual([]);
	});

	it('summarizes', () => {
		expect(quantities([{ commodity_id: 'usd', amount: '-112.50' }], ref)).toEqual([
			{ commodity: usd, quantity: '-112.50' }
		]);
		expect(quantities(undefined, ref)).toEqual([]);
		expect(legName(t.splits[0].from, ref)).toBe('Sam · checking');
		expect(legName(t.splits[0].to, ref)).toBe('Grocer');
		expect(legName({ commodity_id: 'usd', quantity: '12.50' }, ref)).toBe('Unknown');
		expect(categoryNames(t.splits, ref)).toEqual(['Food']);
	});

	it('runs the balance back from the newest', () => {
		const moved = (amount: string) => ({
			...tx([]),
			matched: [{ commodity_id: 'usd', amount }]
		});
		const run = runningBalances(
			[moved('-12.50'), moved('100.00')],
			new Map([['usd', '87.50']]),
			ref
		);
		expect(run.map((r) => r.map((q) => q.quantity))).toEqual([['87.50'], ['100.00']]);
	});
});
