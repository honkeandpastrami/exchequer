// The accounts page: balances grouped by owner, then by holding.

import type { Schemas } from '../api/client';
import { accountKinds, holding, holdings, partyKinds, rank, type Holding } from '../labels';
import { sum } from '../money';

/** `value`: in the household currency, absent when no price connects them. */
export type Line = {
	commodity: Schemas['Commodity'];
	balance: string;
	value?: string;
	valuation?: Schemas['ValuationOut'];
};
/**
 * What lines are worth together in the household currency: the sum of
 * those with a value, the codes of those without, and whether any value
 * came from a stale price.
 */
export type Worth = { value: string; unpriced: string[]; stale: boolean };
export type Row = { account: Schemas['Account']; lines: Line[]; worth: Worth };
export type Section = { holding: Holding; rows: Row[]; totals: Line[]; worth: Worth };
/** `net` is set only when the owner has more than one holding. */
export type Group = {
	owner: Schemas['Party'];
	sections: Section[];
	net: Line[] | null;
	netWorth: Worth | null;
};

const byCode = (a: Line, b: Line) => a.commodity.code.localeCompare(b.commodity.code);

/**
 * One group per owner (household, people, businesses), split by holding
 * (liquid, investments, loans), each account with its balance per commodity.
 * Archived accounts are left out unless asked for; the totals are of the
 * accounts shown.
 */
export function groupBalances(
	parties: Schemas['Party'][],
	commodities: Schemas['Commodity'][],
	accounts: Schemas['Account'][],
	balances: Schemas['AccountBalance'][],
	showArchived: boolean
): Group[] {
	const commodity = new Map(commodities.map((c) => [c.id, c]));
	const lines = new Map<string, Line[]>();
	for (const b of balances) {
		const c = commodity.get(b.commodity_id);
		if (c)
			lines.set(b.account_id, [
				...(lines.get(b.account_id) ?? []),
				{
					commodity: c,
					balance: b.balance,
					value: b.value ?? undefined,
					valuation: b.valuation ?? undefined
				}
			]);
	}
	const owners = [...parties].sort(
		(a, b) => rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)
	);
	return owners
		.map((owner) => {
			const rows = accounts
				.filter((a) => a.owner_id === owner.id)
				.filter((a) => showArchived || !a.archived)
				.sort(
					(a, b) =>
						rank(accountKinds, a.kind) - rank(accountKinds, b.kind) || a.name.localeCompare(b.name)
				)
				.map((account) => {
					const ls = (lines.get(account.id) ?? []).sort(byCode);
					return { account, lines: ls, worth: worth(ls) };
				});
			const sections = (Object.keys(holdings) as Holding[])
				.map((h) => rows.filter((r) => holding(r.account) === h))
				.filter((rs) => rs.length > 0)
				.map((rs) => ({
					holding: holding(rs[0].account),
					rows: rs,
					totals: totals(rs),
					worth: worth(rs.flatMap((r) => r.lines))
				}));
			const many = sections.length > 1;
			return {
				owner,
				sections,
				net: many ? totals(rows) : null,
				netWorth: many ? worth(rows.flatMap((r) => r.lines)) : null
			};
		})
		.filter((g) => g.sections.length > 0);
}

function totals(rows: Row[]): Line[] {
	const by = new Map<string, Line[]>();
	for (const l of rows.flatMap((r) => r.lines)) {
		by.set(l.commodity.id, [...(by.get(l.commodity.id) ?? []), l]);
	}
	return [...by.values()]
		.map((ls) => ({ commodity: ls[0].commodity, balance: sum(ls.map((l) => l.balance)) }))
		.sort(byCode);
}

/** Whether any balance shown isn't in `code`: then values in it are worth a column. */
export function mixed(groups: Group[], code: string): boolean {
	return groups.some((g) =>
		g.sections.some((s) => s.rows.some((r) => r.lines.some((l) => l.commodity.code !== code)))
	);
}

function worth(lines: Line[]): Worth {
	const priced = lines.filter((l) => l.value !== undefined);
	const unpriced = [
		...new Set(lines.filter((l) => l.value === undefined).map((l) => l.commodity.code))
	];
	return {
		value: sum(priced.map((l) => l.value as string)),
		unpriced: unpriced.sort(),
		stale: priced.some((l) => l.valuation?.stale)
	};
}
