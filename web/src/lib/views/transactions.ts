// The transactions page: what each transaction did, in words and amounts.

import type { Schemas } from '../api/client';
import { minus, sum } from '../money';
import { accountLabel } from '../names';
import type { Reference } from '../reference';

type Leg = Schemas['Leg'];
type Split = Schemas['Split'];
export type Quantity = { commodity: Schemas['Commodity'] | undefined; quantity: string };

/** The account or counterparty on one side of a split. */
export function legName(leg: Leg, ref: Reference): string {
	if (leg.account_id) {
		const a = ref.accounts.get(leg.account_id);
		return a ? accountLabel(a, ref) : 'Unknown account';
	}
	if (leg.counterparty_id)
		return ref.counterparties.get(leg.counterparty_id)?.name ?? 'Unknown counterparty';
	return '—';
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

/** How much the transaction moved: the sum of what left each split's source. */
export function size(tx: Schemas['Transaction'], ref: Reference): Quantity[] {
	return totals(
		tx.splits.map((s) => s.from),
		ref
	);
}

/** One line summarizing the splits: `Checking → Grocer`, or `3 splits`. */
export function route(splits: Split[], ref: Reference): string {
	if (splits.length !== 1) return `${splits.length} splits`;
	return `${legName(splits[0].from, ref)} → ${legName(splits[0].to, ref)}`;
}

/** Distinct category names across the splits. */
export function categoryNames(splits: Split[], ref: Reference): string[] {
	const ids = [...new Set(splits.map((s) => s.category_id).filter((id) => id != null))];
	return ids.map((id) => ref.categories.get(id)?.name ?? 'Unknown category');
}

/**
 * The account's balance after each transaction, newest first. `opening` is the
 * balance per commodity ID at the end of the first transaction's day; the
 * list must hold every transaction on the account from that day back.
 */
export function runningBalances(
	txs: Schemas['Transaction'][],
	accountId: string,
	opening: Map<string, string>,
	ref: Reference
): Quantity[][] {
	const balance = new Map(opening);
	return txs.map((tx) => {
		const after = [...balance.entries()]
			.map(([id, quantity]) => ({ commodity: ref.commodities.get(id), quantity }))
			.sort((a, b) => (a.commodity?.code ?? '').localeCompare(b.commodity?.code ?? ''));
		for (const e of effectOn(tx, accountId, ref)) {
			const id = e.commodity?.id;
			if (id) balance.set(id, minus(balance.get(id) ?? '0', e.quantity));
		}
		return after;
	});
}
