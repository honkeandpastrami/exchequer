// A bank row's form in import review (ux.md §10): the row as splits, like
// any transaction's, with the bank account's side fixed. Unsplit, it's one
// split whose fields are the row's labels: yours, or a confident
// suggestion's, or not chosen yet. Saving sends only what changed, so an
// untouched suggestion stays a suggestion.

import type { Schemas } from '../api/client';
import { negate, sign, sum } from '../money';
import type { Reference } from '../reference';
import {
	blankSplit,
	cleanQuantity,
	OUTSIDE,
	preview,
	UNSET,
	type LegDraft,
	type SplitDraft
} from './editor';
import { labelState } from './imports';

type Row = Schemas['StagedRow'];

export type RowDraft = { description: string; splits: SplitDraft[] };

/** Where a bank row is: its statement's account and commodity. */
export type Bank = { account: string; commodity: string };

/** The row's amount, unsigned. */
export const whole = (row: Row) => (sign(row.amount) < 0 ? negate(row.amount) : row.amount);

/** The bank account's side: money leaving it is `from`. */
export const fixedSide = (row: Row): 'from' | 'to' => (sign(row.amount) < 0 ? 'from' : 'to');

const leg = (account: string, outside = ''): LegDraft => ({ account, outside });

function otherLeg(
	ref: Reference,
	account: string | null | undefined,
	counterparty: string | null | undefined
): LegDraft {
	if (account) return leg(account);
	return leg(OUTSIDE, counterparty ? (ref.counterparties.get(counterparty)?.name ?? '') : '');
}

function line(row: Row, bank: Bank, other: LegDraft, more: Partial<SplitDraft>): SplitDraft {
	const s = { ...blankSplit(bank.commodity), ...more };
	const mine = leg(bank.account);
	return fixedSide(row) === 'from'
		? { ...s, from: mine, to: other }
		: { ...s, from: other, to: mine };
}

/** What a label shows: yours, a confident suggestion's, or `undefined`. */
function shown(row: Row, label: Schemas['Label']): { value: string | null } | undefined {
	const st = labelState(row, label);
	if (st.kind === 'yours') {
		const v =
			label === 'counterparty'
				? row.counterparty_id
				: label === 'category'
					? row.category_id
					: row.purpose_id;
		return { value: v ?? null };
	}
	if (st.kind === 'confident') return { value: st.suggestion.value ?? null };
	return undefined;
}

/** The row's form as it is now. */
export function rowDraft(row: Row, bank: Bank, ref: Reference): RowDraft {
	const description = row.set_description ?? '';
	if (row.splits.length)
		return {
			description,
			splits: row.splits.map((p) =>
				line(row, bank, otherLeg(ref, p.transfer_account_id, p.counterparty_id), {
					quantity: p.amount,
					category: p.category_id ?? UNSET,
					purpose: p.purpose_id ?? UNSET,
					description: p.description ?? ''
				})
			)
		};
	// A transfer, chosen or suggested, or its match's other side.
	const side = shown(row, 'counterparty');
	const transfer = row.transfer_account_id;
	const other = transfer ? leg(transfer) : side ? otherLeg(ref, null, side.value) : leg('');
	// A purpose names someone and a category something: none is not chosen
	// yet.
	const label = (l: Schemas['Label']) => {
		const v = shown(row, l);
		return !v || !v.value ? UNSET : v.value;
	};
	return {
		description,
		splits: [
			line(row, bank, other, {
				quantity: whole(row),
				category: label('category'),
				purpose: label('purpose')
			})
		]
	};
}

const key = (name: string) => name.trim().toLowerCase();

/** The other side of a split: its account, or its counterparty by name. */
const otherOf = (row: Row, s: SplitDraft) => (fixedSide(row) === 'from' ? s.to : s.from);

function sideUpdate(
	l: LegDraft,
	ids: Map<string, string>
): { transfer_account_id: string } | { counterparty_id: string | null } {
	if (l.account !== OUTSIDE) return { transfer_account_id: l.account };
	return { counterparty_id: l.outside.trim() ? (ids.get(key(l.outside)) ?? null) : null };
}

const same = (a: LegDraft, b: LegDraft) =>
	a.account === b.account && (a.account !== OUTSIDE || key(a.outside) === key(b.outside));

/**
 * What to send for `after`, given the form as it opened (`before`):
 * counterparty names must all exist in `ref` (create them first).
 */
export function rowUpdate(
	row: Row,
	before: RowDraft,
	after: RowDraft,
	ref: Reference
): Schemas['RowUpdate'] {
	const ids = new Map([...ref.counterparties.values()].map((c) => [key(c.name), c.id]));
	const out: Schemas['RowUpdate'] = {};
	const description = after.description.trim();
	if (description !== (row.set_description ?? '')) out.set_description = description || null;
	const value = (v: string) => (v === UNSET ? undefined : v || null);
	if (after.splits.length > 1) {
		out.splits = after.splits.map((s) => {
			const other = sideUpdate(otherOf(row, s), ids);
			// Fields the split can't have (a transfer's) aren't sent.
			const p = preview(s, ref);
			return {
				amount: cleanQuantity(s.quantity),
				...('transfer_account_id' in other
					? { transfer_account_id: other.transfer_account_id }
					: other.counterparty_id
						? { counterparty_id: other.counterparty_id }
						: {}),
				category_id: (p.category && value(s.category)) || null,
				purpose_id: (p.purpose && value(s.purpose)) || null,
				description: s.description.trim() || null
			};
		});
		return out;
	}
	// One split: the row's own labels, those you changed (all, if it was split).
	const unsplit = row.splits.length > 0;
	if (unsplit) out.splits = null;
	const [a, b] = [after.splits[0], before.splits[0]];
	const other = otherOf(row, a);
	const p = other.account ? preview(a, ref) : { category: true, purpose: true };
	if (other.account && (unsplit || !same(other, otherOf(row, b))))
		Object.assign(out, sideUpdate(other, ids));
	if (p.category && a.category && a.category !== UNSET && (unsplit || a.category !== b.category))
		out.category_id = a.category;
	if (p.purpose && a.purpose && a.purpose !== UNSET && (unsplit || a.purpose !== b.purpose))
		out.purpose_id = a.purpose;
	return out;
}

/** Problems the form can point out before asking the server: a split row's
 * parts need every field chosen, and must add up to the bank's amount. */
export function rowProblems(row: Row, d: RowDraft, ref: Reference): string[] {
	if (d.splits.length < 2) return [];
	const out: string[] = [];
	const amounts: string[] = [];
	d.splits.forEach((s, i) => {
		const n = `Split ${i + 1}: `;
		const other = otherOf(row, s);
		if (!other.account) out.push(`${n}choose the other side.`);
		const q = cleanQuantity(s.quantity);
		if (!/^\d+(\.\d+)?$/.test(q) || /^[0.]+$/.test(q)) out.push(`${n}add an amount.`);
		else amounts.push(q);
		const p = preview(s, ref);
		if (other.account && p.purpose && (!s.category || s.category === UNSET))
			out.push(`${n}choose a category.`);
		if (other.account && p.purpose && (!s.purpose || s.purpose === UNSET))
			out.push(`${n}choose who it's for.`);
	});
	if (amounts.length === d.splits.length && sign(sum([...amounts, negate(whole(row))])) !== 0)
		out.push(`The splits add up to ${sum(amounts)}; the bank's amount is ${whole(row)}.`);
	return out;
}
