// The accounts page: balances grouped by owner, then by holding.

import type { Schemas } from '../api/client';
import { accountKinds, holding, holdings, partyKinds, rank, type Holding } from '../labels';
import { sum } from '../money';

export type Line = { commodity: Schemas['Commodity']; balance: string };
export type Row = { account: Schemas['Account']; lines: Line[] };
export type Section = { holding: Holding; rows: Row[]; totals: Line[] };
/** `net` is set only when the owner has more than one holding. */
export type Group = { owner: Schemas['Party']; sections: Section[]; net: Line[] | null };

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
				{ commodity: c, balance: b.balance }
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
				.map((account) => ({ account, lines: (lines.get(account.id) ?? []).sort(byCode) }));
			const sections = (Object.keys(holdings) as Holding[])
				.map((h) => rows.filter((r) => holding(r.account) === h))
				.filter((rs) => rs.length > 0)
				.map((rs) => ({ holding: holding(rs[0].account), rows: rs, totals: totals(rs) }));
			return { owner, sections, net: sections.length > 1 ? totals(rows) : null };
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
