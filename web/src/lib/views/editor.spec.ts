import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import type { Reference } from '../reference';
import {
	OUTSIDE,
	blankDraft,
	blankSplit,
	fromTransaction,
	newCounterparties,
	preview,
	problems,
	toRequest,
	type SplitDraft
} from './editor';

const acct = (id: string, owner: string, liquid = true): Schemas['Account'] => ({
	id,
	name: id,
	owner_id: owner,
	kind: liquid ? 'checking' : 'brokerage',
	liquid,
	archived: false
});
const ref: Reference = {
	parties: new Map(),
	commodities: new Map([
		['usd', { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 }],
		['eur', { id: 'eur', code: 'EUR', kind: 'fiat', precision: 2 }]
	]),
	accounts: new Map([
		['alex', acct('alex', 'p-alex')],
		['sam', acct('sam', 'p-sam')],
		['alex2', acct('alex2', 'p-alex')],
		['broker', acct('broker', 'p-alex', false)]
	]),
	counterparties: new Map([['grocer', { id: 'grocer', name: 'Grocer' }]]),
	categories: new Map(),
	tags: new Map()
};

const split = (from: string, to: string, more: Partial<SplitDraft> = {}): SplitDraft => {
	const leg = (x: string) =>
		ref.accounts.has(x) ? { account: x, outside: '' } : { account: OUTSIDE, outside: x };
	return { ...blankSplit('usd'), from: leg(from), to: leg(to), quantity: '12.50', ...more };
};

describe('editor preview', () => {
	it('derives the kind and the fields it allows', () => {
		expect(preview(split('alex', 'Grocer'), ref)).toEqual({
			kind: 'expense',
			liquidation: false,
			category: true,
			purpose: true,
			settles: false
		});
		expect(preview(split('Grocer', 'alex'), ref).kind).toBe('income');
		expect(preview(split('alex', 'alex2'), ref)).toMatchObject({
			kind: 'transfer',
			category: false,
			settles: false
		});
		expect(preview(split('alex', 'sam'), ref).settles).toBe(true);
		expect(preview(split('broker', 'alex'), ref)).toMatchObject({
			liquidation: true,
			category: true,
			purpose: true
		});
		expect(preview(split('broker', 'Grocer'), ref)).toMatchObject({
			category: true,
			purpose: false
		});
		expect(preview(split('alex', 'alex', { converted: true, toCommodity: 'eur' }), ref).kind).toBe(
			'conversion'
		);
		expect(preview(split('alex', 'alex'), ref).kind).toBeNull();
		expect(preview(split('Grocer', 'Someone'), ref).kind).toBeNull();
	});
});

describe('editor checks', () => {
	it('points out what the server would refuse', () => {
		const d = { ...blankDraft('usd', '2026-03-01'), description: 'Lunch' };
		d.splits = [split('alex', 'Grocer', { quantity: '1,234.50' })];
		expect(problems(d, ref)).toEqual([]);
		d.splits = [
			split('alex', 'Grocer', { quantity: '1.234' }),
			{ ...split('alex', 'Grocer', { quantity: '0' }), to: { account: '', outside: '' } }
		];
		expect(problems(d, ref)).toEqual([
			'Split 1: USD has 2 decimal places.',
			'Split 2: choose both sides, at least one of them your account.',
			'Split 2: the amount must be more than zero.'
		]);
	});
});

describe('editor request', () => {
	it('drops fields the split cannot have and resolves names', () => {
		const d = { ...blankDraft('usd', '2026-03-01'), description: ' Lunch ', tags: 'a, b,,' };
		d.splits = [
			split('alex', 'grocer', { quantity: '1,000.00', category: 'food', purpose: 'p-sam' }),
			split('alex', 'alex2', { category: 'food', settles: '2026-02' })
		];
		const r = toRequest(d, ref);
		expect(r.description).toBe('Lunch');
		expect(r.tags).toEqual(['a', 'b']);
		expect(r.splits[0]).toEqual({
			from: { account_id: 'alex', commodity_id: 'usd', quantity: '1000.00' },
			to: { counterparty_id: 'grocer', commodity_id: 'usd', quantity: '1000.00' },
			category_id: 'food',
			purpose_id: 'p-sam',
			settles: null,
			description: null
		});
		expect(r.splits[1]).toMatchObject({ category_id: null, settles: null });
	});

	it('leaves an unnamed outside side unknown', () => {
		const d = { ...blankDraft('usd', '2026-03-01'), description: 'Cash' };
		d.splits = [split('alex', '  ', { category: 'food' })];
		expect(problems(d, ref)).toEqual([]);
		expect(preview(d.splits[0], ref).kind).toBe('expense');
		expect(newCounterparties(d, ref)).toEqual([]);
		expect(toRequest(d, ref).splits[0].to).toEqual({ commodity_id: 'usd', quantity: '12.50' });
	});

	it('round-trips a transaction', () => {
		const tx: Schemas['Transaction'] = {
			id: 't',
			date: '2026-03-01',
			description: 'Euros',
			notes: null,
			tags: ['trip'],
			import_batch_id: 'b',
			external_refs: [{ source: 'bank', ref: '1' }],
			splits: [
				{
					from: { account_id: 'alex', commodity_id: 'usd', quantity: '110.00' },
					to: { account_id: 'alex2', commodity_id: 'eur', quantity: '100.00' },
					kind: 'conversion',
					liquidation: false
				}
			]
		};
		const r = toRequest(fromTransaction(tx, ref), ref);
		expect(r).toEqual({
			date: tx.date,
			description: tx.description,
			notes: null,
			tags: ['trip'],
			external_refs: tx.external_refs,
			splits: [
				{
					from: tx.splits[0].from,
					to: tx.splits[0].to,
					category_id: null,
					purpose_id: null,
					settles: null,
					description: null
				}
			]
		});
	});

	it('lists counterparties to create', () => {
		const d = blankDraft('usd', '2026-03-01');
		d.splits = [split('alex', 'grocer'), split('alex', 'New Shop'), split('new shop ', 'alex')];
		expect(newCounterparties(d, ref)).toEqual(['New Shop']);
	});
});
