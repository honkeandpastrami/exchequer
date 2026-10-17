import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import { holding } from '../labels';
import { groupBalances } from './accounts';

const usd: Schemas['Commodity'] = { id: 'usd', code: 'USD', kind: 'fiat', precision: 2 };
const parties: Schemas['Party'][] = [
	{ id: 'sam', name: 'Sam', kind: 'person' },
	{ id: 'home', name: 'Household', kind: 'household' },
	{ id: 'studio', name: 'Studio', kind: 'business' }
];
const account = (
	id: string,
	owner_id: string,
	kind: Schemas['AccountKind'],
	archived = false,
	liquid = kind !== 'brokerage'
) => ({
	id,
	name: id,
	owner_id,
	kind,
	liquid,
	archived,
	numbers: []
});
const accounts: Schemas['Account'][] = [
	account('card', 'sam', 'credit_card'),
	account('checking', 'sam', 'checking'),
	account('old', 'sam', 'savings', true),
	account('joint', 'home', 'savings'),
	account('stocks', 'home', 'brokerage'),
	account('house', 'home', 'mortgage')
];
const balances: Schemas['AccountBalance'][] = [
	{ account_id: 'checking', commodity_id: 'usd', balance: '100.10' },
	{ account_id: 'card', commodity_id: 'usd', balance: '-40.05' },
	{ account_id: 'joint', commodity_id: 'usd', balance: '5.00' },
	{ account_id: 'old', commodity_id: 'usd', balance: '1.00' },
	{ account_id: 'stocks', commodity_id: 'usd', balance: '20.00' },
	{ account_id: 'house', commodity_id: 'usd', balance: '-300.00' }
];
const ids = (g: { sections: { rows: { account: { id: string } }[] }[] }) =>
	g.sections.flatMap((s) => s.rows.map((r) => r.account.id));

describe('groupBalances', () => {
	it('groups by owner, household first, and totals exactly', () => {
		const g = groupBalances(parties, [usd], accounts, balances, false);
		expect(g.map((x) => x.owner.name)).toEqual(['Household', 'Sam']);
		expect(ids(g[1])).toEqual(['checking', 'card']);
		expect(g[1].sections.map((s) => [s.holding, s.totals])).toEqual([
			['liquid', [{ commodity: usd, balance: '60.05' }]]
		]);
		expect(g[1].net, 'one holding: no net').toBeNull();
	});

	it('keeps liquid, investments and loans apart, mixed only in the net', () => {
		const home = groupBalances(parties, [usd], accounts, balances, false)[0];
		expect(
			home.sections.map((s) => [s.holding, s.rows.map((r) => r.account.id), s.totals])
		).toEqual([
			['liquid', ['joint'], [{ commodity: usd, balance: '5.00' }]],
			['investments', ['stocks'], [{ commodity: usd, balance: '20.00' }]],
			['loans', ['house'], [{ commodity: usd, balance: '-300.00' }]]
		]);
		expect(home.net).toEqual([{ commodity: usd, balance: '-275.00' }]);
	});

	it('puts a mortgage under loans whatever its liquid flag', () => {
		expect(holding({ kind: 'mortgage', liquid: true })).toBe('loans');
		expect(holding({ kind: 'mortgage', liquid: false })).toBe('loans');
		expect(holding({ kind: 'savings', liquid: false })).toBe('investments');
		expect(holding({ kind: 'credit_card', liquid: true })).toBe('liquid');
	});

	it('hides archived accounts unless asked, even with a balance', () => {
		const sam = (archived: boolean) =>
			groupBalances(parties, [usd], accounts, balances, archived)[1];
		expect(ids(sam(false))).not.toContain('old');
		expect(sam(false).sections[0].totals).toEqual([{ commodity: usd, balance: '60.05' }]);
		expect(ids(sam(true))).toContain('old');
		expect(sam(true).sections[0].totals).toEqual([{ commodity: usd, balance: '61.05' }]);
	});
});
