import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import type { Reference } from '../reference';
import { draftFrom, paramProblems, toParams, unsaved } from './params';

const party = (id: string, kind: Schemas['PartyKind'], more = {}): Schemas['Party'] => ({
	id,
	name: id.toUpperCase(),
	kind,
	...more
});
const ref: Reference = {
	parties: new Map([
		['home', party('home', 'household')],
		['sam', party('sam', 'person')],
		['alex', party('alex', 'person')],
		['studio', party('studio', 'business')]
	]),
	commodities: new Map([['usd', { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 }]]),
	accounts: new Map(),
	counterparties: new Map(),
	categories: new Map(),
	tags: new Map()
};

const settlement = (more: Partial<Schemas['Settlement']> = {}): Schemas['Settlement'] => ({
	month: '2026-03',
	closed: false,
	stipends: [],
	funders: [],
	...more
});

describe('settlement parameters', () => {
	it('starts from what is in force, households funding by default', () => {
		const s = settlement({
			params_from: '2026-01',
			stipends: [{ party_id: 'alex', amount: '650.00' }]
		});
		expect(draftFrom(s, ref)).toEqual({
			stipends: { alex: '650.00', sam: '' },
			funders: { studio: 'home' }
		});
	});

	it('asks for every stipend and checks the amounts', () => {
		const d = { stipends: { alex: '1,000.5', sam: '' }, funders: { studio: 'home' } };
		expect(paramProblems(d, ref)).toEqual(['Stipend for SAM is missing (0 is fine).']);
		d.stipends.sam = '-3';
		expect(paramProblems(d, ref)).toEqual(['Stipend for SAM: "-3" isn\'t an amount.']);
		d.stipends.sam = '0.001';
		expect(paramProblems(d, ref)).toEqual(['Stipend for SAM: USD has 2 decimal places.']);
		d.stipends.sam = '0';
		expect(paramProblems(d, ref)).toEqual([]);
		expect(toParams(d, ref)).toEqual({
			stipends: [
				{ party_id: 'alex', amount: '1000.5' },
				{ party_id: 'sam', amount: '0' }
			],
			funders: [{ business_id: 'studio', funder_id: 'home' }]
		});
	});

	it('needs the USD commodity', () => {
		const noUsd = { ...ref, commodities: new Map() };
		expect(paramProblems({ stipends: {}, funders: {} }, noUsd)).toHaveLength(1);
	});

	it('treats carried-forward parameters as not saved for the month', () => {
		const stored = {
			stipends: [
				{ party_id: 'alex', amount: '650.00' },
				{ party_id: 'sam', amount: '0.00' }
			],
			funders: [{ business_id: 'studio', funder_id: 'sam' }]
		};
		const carried = settlement({ params_from: '2026-01', ...stored });
		expect(unsaved(draftFrom(carried, ref), carried, ref)).toBe(true);

		const own = settlement({ params_from: '2026-03', ...stored });
		const d = draftFrom(own, ref);
		expect(unsaved(d, own, ref)).toBe(false);
		d.stipends.alex = '650';
		expect(unsaved(d, own, ref)).toBe(false);
		d.funders.studio = 'home';
		expect(unsaved(d, own, ref)).toBe(true);
	});
});
