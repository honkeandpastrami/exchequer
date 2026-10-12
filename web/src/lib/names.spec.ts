import { describe, expect, it } from 'vitest';
import type { Schemas } from './api/client';
import { accountLabel, accountsByOwner } from './names';
import type { Reference } from './reference';

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
	parties: new Map<string, Schemas['Party']>([
		['studio', { id: 'studio', name: 'Studio', kind: 'business' }],
		['sam', { id: 'sam', name: 'Sam', kind: 'person' }],
		['alex', { id: 'alex', name: 'Alex', kind: 'person' }],
		['home', { id: 'home', name: 'Household', kind: 'household' }]
	]),
	commodities: new Map(),
	accounts: new Map([
		['a1', acct('Checking', 'studio')],
		['a2', acct('Savings', 'sam')],
		['a3', acct('Checking', 'sam')],
		['a4', acct('Checking', 'alex')],
		['a5', acct('Joint', 'home')]
	]),
	counterparties: new Map(),
	categories: new Map(),
	tags: new Map()
};

describe('accounts with their owners', () => {
	it('labels and orders accounts by owner', () => {
		expect(accountsByOwner(ref).map((a) => accountLabel(a, ref))).toEqual([
			'Household · Joint',
			'Alex · Checking',
			'Sam · Checking',
			'Sam · Savings',
			'Studio · Checking'
		]);
	});
});
