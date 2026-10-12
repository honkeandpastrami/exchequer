import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import { groupBalances } from './accounts';

const usd: Schemas['Commodity'] = { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 };
const parties: Schemas['Party'][] = [
	{ id: 'sam', name: 'Sam', kind: 'person' },
	{ id: 'home', name: 'Household', kind: 'household' },
	{ id: 'studio', name: 'Studio', kind: 'business' }
];
const account = (id: string, owner_id: string, kind: Schemas['AccountKind'], archived = false) => ({
	id,
	name: id,
	owner_id,
	kind,
	liquid: true,
	archived
});
const accounts: Schemas['Account'][] = [
	account('card', 'sam', 'credit_card'),
	account('checking', 'sam', 'checking'),
	account('old', 'sam', 'savings', true),
	account('joint', 'home', 'savings')
];
const balances: Schemas['AccountBalance'][] = [
	{ account_id: 'checking', commodity_id: 'usd', balance: '100.10' },
	{ account_id: 'card', commodity_id: 'usd', balance: '-40.05' },
	{ account_id: 'joint', commodity_id: 'usd', balance: '5.00' },
	{ account_id: 'old', commodity_id: 'usd', balance: '1.00' }
];

describe('groupBalances', () => {
	it('groups by owner, household first, and totals exactly', () => {
		const g = groupBalances(parties, [usd], accounts, balances, false);
		expect(g.map((x) => x.owner.name)).toEqual(['Household', 'Sam']);
		expect(g[1].rows.map((r) => r.account.id)).toEqual(['checking', 'card']);
		expect(g[1].totals).toEqual([{ commodity: usd, balance: '60.05' }]);
	});

	it('hides archived accounts unless asked, even with a balance', () => {
		const sam = (archived: boolean) =>
			groupBalances(parties, [usd], accounts, balances, archived)[1];
		expect(sam(false).rows.map((r) => r.account.id)).not.toContain('old');
		expect(sam(false).totals).toEqual([{ commodity: usd, balance: '60.05' }]);
		expect(sam(true).rows.map((r) => r.account.id)).toContain('old');
		expect(sam(true).totals).toEqual([{ commodity: usd, balance: '61.05' }]);
	});
});
