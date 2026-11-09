// The transactions page: what each transaction did, in words and amounts.

import type { Schemas } from '../api/client';
import { minus, sum } from '../money';
import { accountLabel } from '../names';
import type { Reference } from '../reference';

type Leg = Schemas['Leg'];
type Split = Schemas['Split'];
export type Quantity = { commodity: Schemas['Commodity'] | undefined; quantity: string };

/** What to call an outside side with no counterparty. */
export const UNKNOWN_OUTSIDE = 'Unknown';

/** The account or counterparty on one side of a split. */
export function legName(leg: Leg, ref: Reference): string {
	if (leg.account_id) {
		const a = ref.accounts.get(leg.account_id);
		return a ? accountLabel(a, ref) : 'Unknown account';
	}
	if (leg.counterparty_id)
		return ref.counterparties.get(leg.counterparty_id)?.name ?? 'Unknown counterparty';
	return UNKNOWN_OUTSIDE;
}

function totals(entries: { commodity_id: string; quantity: string }[], ref: Reference): Quantity[] {
	const by = new Map<string, string[]>();
	for (const e of entries) by.set(e.commodity_id, [...(by.get(e.commodity_id) ?? []), e.quantity]);
	return [...by.entries()].map(([id, qs]) => ({
		commodity: ref.commodities.get(id),
		quantity: sum(qs)
	}));
}

/**
 * The transaction's net effect on one account, per commodity: positive into
 * it, negative out of it.
 */
export function effectOn(
	tx: Schemas['Transaction'],
	accountId: string,
	ref: Reference
): Quantity[] {
	const entries = tx.splits.flatMap((s) => [
		...(s.to.account_id === accountId ? [s.to] : []),
		...(s.from.account_id === accountId ? [{ ...s.from, quantity: `-${s.from.quantity}` }] : [])
	]);
	return totals(entries, ref).filter((q) => !/^-?0(\.0*)?$/.test(q.quantity));
}

/** Amounts as the API gives them (`matched`, `totals`), with their commodities. */
export function quantities(
	amounts: Schemas['CommodityAmount'][] | null | undefined,
	ref: Reference
): Quantity[] {
	return (amounts ?? []).map((a) => ({
		commodity: ref.commodities.get(a.commodity_id),
		quantity: a.amount
	}));
}

/** Distinct category names across the splits. */
export function categoryNames(splits: Split[], ref: Reference): string[] {
	const ids = [...new Set(splits.map((s) => s.category_id).filter((id) => id != null))];
	return ids.map((id) => ref.categories.get(id)?.name ?? 'Unknown category');
}

/**
 * The balance after each transaction, newest first, from what each one moved
 * (`matched` of one account's history). `opening` is the balance per
 * commodity ID at the end of the first transaction's day; the list must hold
 * every transaction on the account from that day back.
 */
export function runningBalances(
	txs: Schemas['Transaction'][],
	opening: Map<string, string>,
	ref: Reference
): Quantity[][] {
	const balance = new Map(opening);
	return txs.map((tx) => {
		const after = [...balance.entries()]
			.map(([id, quantity]) => ({ commodity: ref.commodities.get(id), quantity }))
			.sort((a, b) => (a.commodity?.code ?? '').localeCompare(b.commodity?.code ?? ''));
		for (const m of tx.matched ?? []) {
			balance.set(m.commodity_id, minus(balance.get(m.commodity_id) ?? '0', m.amount));
		}
		return after;
	});
}
