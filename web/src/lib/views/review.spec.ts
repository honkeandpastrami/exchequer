import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import type { Reference } from '../reference';
import { OUTSIDE, UNSET, blankSplit } from './editor';
import { rowDraft, rowProblems, rowUpdate } from './review';

const acct = (id: string, owner: string): Schemas['Account'] => ({
	id,
	name: id,
	owner_id: owner,
	kind: 'checking',
	liquid: true,
	archived: false,
	numbers: []
});
const ref: Reference = {
	parties: new Map([['home', { id: 'home', name: 'Home', kind: 'household' }]]),
	commodities: new Map([['usd', { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 }]]),
	accounts: new Map([
		['checking', acct('checking', 'home')],
		['escrow', acct('escrow', 'home')]
	]),
	counterparties: new Map([['lender', { id: 'lender', name: 'Lender' }]]),
	categories: new Map([
		['interest', { id: 'interest', name: 'Interest' }],
		['principal', { id: 'principal', name: 'Principal' }]
	]),
	tags: new Map()
};
const bank = { account: 'checking', commodity: 'usd' };

const suggestion = (
	label: Schemas['Label'],
	value: string,
	confident: boolean
): Schemas['LabelSuggestion'] => ({
	label,
	value,
	confidence: confident ? 950 : 600,
	confident,
	reason: 'like before',
	suggester: { id: 's', kind: 'local', name: 'lexical-knn', version: '1', details: {} }
});

const row = (more: Partial<Schemas['StagedRow']> = {}): Schemas['StagedRow'] => ({
	statement: 0,
	position: 3,
	date: '2026-10-01',
	amount: '-1900.00',
	description: 'MORTGAGE PMT',
	status: 'new',
	decided: false,
	decision: 'new',
	raw: {},
	chosen: [],
	suggestions: [],
	missing: [],
	not_applicable: [],
	splits: [],
	...more
});

describe('a bank row as a form', () => {
	it('shows yours and confident suggestions, and leaves the rest to choose', () => {
		const r = row({
			chosen: ['purpose'],
			purpose_id: 'home',
			suggestions: [suggestion('counterparty', 'lender', true), suggestion('category', 'x', false)]
		});
		const d = rowDraft(r, bank, ref);
		expect(d.splits).toHaveLength(1);
		const [s] = d.splits;
		expect(s.from).toEqual({ account: 'checking', outside: '' });
		expect(s.to).toEqual({ account: OUTSIDE, outside: 'Lender' });
		expect([s.quantity, s.category, s.purpose]).toEqual(['1900.00', UNSET, 'home']);
	});

	it('sends only what you changed, so a suggestion stays one', () => {
		const r = row({ suggestions: [suggestion('counterparty', 'lender', true)] });
		const before = rowDraft(r, bank, ref);
		const after = structuredClone(before);
		expect(rowUpdate(r, before, after, ref)).toEqual({});
		after.splits[0].category = 'principal';
		after.description = ' Monthly payment ';
		expect(rowUpdate(r, before, after, ref)).toEqual({
			set_description: 'Monthly payment',
			category_id: 'principal'
		});
		after.splits[0].to = { account: 'escrow', outside: '' };
		expect(rowUpdate(r, before, after, ref)).toMatchObject({ transfer_account_id: 'escrow' });
	});

	it('splits into parts that must add up to the bank amount', () => {
		const r = row();
		const before = rowDraft(r, bank, ref);
		const after = structuredClone(before);
		after.splits[0] = {
			...after.splits[0],
			to: { account: OUTSIDE, outside: 'lender' },
			quantity: '1,200.00',
			category: 'principal',
			purpose: 'home'
		};
		after.splits.push({ ...blankSplit('usd', after.splits[0]), quantity: '500.00' });
		after.splits.push({
			...blankSplit('usd', after.splits[0]),
			to: { account: 'escrow', outside: '' },
			quantity: '100.00'
		});
		expect(rowProblems(r, after, ref)).toEqual([
			"The splits add up to 1800.00; the bank's amount is 1900.00."
		]);
		after.splits[2].quantity = '200.00';
		expect(rowProblems(r, after, ref)).toEqual([]);
		expect(rowUpdate(r, before, after, ref)).toEqual({
			splits: [
				{
					amount: '1200.00',
					counterparty_id: 'lender',
					category_id: 'principal',
					purpose_id: 'home',
					description: null
				},
				{
					amount: '500.00',
					counterparty_id: 'lender',
					category_id: 'principal',
					purpose_id: 'home',
					description: null
				},
				{
					// A transfer: what it copied from the line above isn't sent.
					amount: '200.00',
					transfer_account_id: 'escrow',
					category_id: null,
					purpose_id: null,
					description: null
				}
			]
		});
		after.splits[1].to = { account: '', outside: '' };
		after.splits[1].category = UNSET;
		expect(rowProblems(r, after, ref)).toEqual(['Split 2: choose the other side.']);
	});

	it('unsplits into the row with every label yours', () => {
		const r = row({
			splits: [
				{
					amount: '1000.00',
					counterparty_id: 'lender',
					category_id: 'principal',
					purpose_id: 'home'
				},
				{ amount: '900.00', transfer_account_id: 'escrow' }
			]
		});
		const before = rowDraft(r, bank, ref);
		expect(before.splits.map((s) => [s.quantity, s.to.account, s.category])).toEqual([
			['1000.00', OUTSIDE, 'principal'],
			['900.00', 'escrow', UNSET]
		]);
		const after = structuredClone(before);
		after.splits.pop();
		expect(rowUpdate(r, before, after, ref)).toEqual({
			splits: null,
			counterparty_id: 'lender',
			category_id: 'principal',
			purpose_id: 'home'
		});
	});
});
