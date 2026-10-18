// The transaction editor: a draft the form binds to, converted to and from
// the API's shape. The server checks every ledger rule; the preview here only
// decides which fields to show, mirroring `domain::transaction`.

import type { Schemas } from '../api/client';
import { sum } from '../money';
import { accountsByOwner, accountLabel } from '../names';
import type { Reference } from '../reference';

/** The select value meaning "someone outside the household". */
export const OUTSIDE = 'outside';
/** A category or purpose not chosen yet (a bank row's, while no one has). */
export const UNSET = '?';

/** One side: an account ID, or `OUTSIDE` with a counterparty name. */
export type LegDraft = { account: string; outside: string };

export type SplitDraft = {
	from: LegDraft;
	to: LegDraft;
	commodity: string;
	quantity: string;
	/** The `to` side receives another commodity (a conversion). */
	converted: boolean;
	toCommodity: string;
	toQuantity: string;
	category: string;
	purpose: string;
	description: string;
	/** `YYYY-MM`, settlement payments only. */
	settles: string;
};

export type Draft = {
	date: string;
	description: string;
	notes: string;
	/** Comma-separated. */
	tags: string;
	splits: SplitDraft[];
	/** Kept as they are: the editor doesn't change them. */
	externalRefs: Schemas['ExternalRef'][];
};

export type Preview = {
	kind: Schemas['Kind'] | null;
	category: boolean;
	/** Whether it takes a purpose, which must then be chosen (design §4.1). */
	purpose: boolean;
	settles: boolean;
};

const leg = (): LegDraft => ({ account: '', outside: '' });

export function blankSplit(commodity: string, like?: SplitDraft): SplitDraft {
	return {
		from: like ? { ...like.from } : leg(),
		to: like ? { ...like.to } : leg(),
		commodity: like?.commodity ?? commodity,
		quantity: '',
		converted: false,
		toCommodity: like?.commodity ?? commodity,
		toQuantity: '',
		category: like?.category ?? '',
		purpose: like?.purpose ?? '',
		description: '',
		settles: ''
	};
}

export function blankDraft(commodity: string, date: string): Draft {
	return {
		date,
		description: '',
		notes: '',
		tags: '',
		splits: [blankSplit(commodity)],
		externalRefs: []
	};
}

function legDraft(l: Schemas['Leg'], ref: Reference): LegDraft {
	if (l.account_id) return { account: l.account_id, outside: '' };
	return { account: OUTSIDE, outside: ref.counterparties.get(l.counterparty_id ?? '')?.name ?? '' };
}

export function fromTransaction(tx: Schemas['Transaction'], ref: Reference): Draft {
	return {
		date: tx.date,
		description: tx.description,
		notes: tx.notes ?? '',
		tags: tx.tags.join(', '),
		externalRefs: tx.external_refs,
		splits: tx.splits.map((s) => ({
			from: legDraft(s.from, ref),
			to: legDraft(s.to, ref),
			commodity: s.from.commodity_id,
			quantity: s.from.quantity,
			converted: s.from.commodity_id !== s.to.commodity_id,
			toCommodity: s.to.commodity_id,
			toQuantity: s.to.quantity,
			category: s.category_id ?? '',
			purpose: s.purpose_id ?? '',
			description: s.description ?? '',
			settles: s.settles ?? ''
		}))
	};
}

/** What the split would be, and which optional fields it can have. */
export function preview(s: SplitDraft, ref: Reference): Preview {
	const none: Preview = {
		kind: null,
		category: false,
		purpose: false,
		settles: false
	};
	const account = (l: LegDraft) =>
		l.account === OUTSIDE ? undefined : ref.accounts.get(l.account);
	// Outside with no name: outside the household, unknown who.
	const set = (l: LegDraft) => l.account === OUTSIDE || !!account(l);
	if (!set(s.from) || !set(s.to)) return none;
	const a = account(s.from);
	const b = account(s.to);
	const same = !s.converted || s.commodity === s.toCommodity;
	let kind: Schemas['Kind'];
	if (a && b) {
		if (same && a.id === b.id) return none;
		// Into or out of an investment or a loan: an expense or income.
		if (a.liquid !== b.liquid) kind = a.liquid ? 'expense' : 'income';
		else kind = same ? 'transfer' : 'conversion';
	} else if (a || b) {
		// With a system counterparty: an opening balance or a reconciliation.
		const system = (l: LegDraft) =>
			l.account === OUTSIDE &&
			[...ref.counterparties.values()].some(
				(c) => c.system && c.name.toLowerCase() === l.outside.trim().toLowerCase()
			);
		kind = system(s.from) || system(s.to) ? 'adjustment' : a ? 'expense' : 'income';
	} else return none;
	const flow = kind === 'expense' || kind === 'income';
	return {
		kind,
		category: flow,
		purpose: flow && (!!a?.liquid || !!b?.liquid),
		settles: kind === 'transfer' && !!a && !!b && a.liquid && b.liquid && a.owner_id !== b.owner_id
	};
}

/** `1,234.50 ` → `1234.50`. */
export function cleanQuantity(q: string): string {
	return q.replace(/[,\s]/g, '');
}

/** What the splits send, per commodity, counting only the amounts already typed. */
export function splitTotals(splits: SplitDraft[]): { commodity: string; value: string }[] {
	const by = new Map<string, string[]>();
	for (const s of splits) {
		const q = cleanQuantity(s.quantity);
		if (/^\d+(\.\d+)?$/.test(q)) by.set(s.commodity, [...(by.get(s.commodity) ?? []), q]);
	}
	return [...by].map(([commodity, qs]) => ({ commodity, value: sum(qs) }));
}

function quantityProblem(q: string, commodity: string, ref: Reference): string | null {
	const c = ref.commodities.get(commodity);
	if (!c) return 'pick a commodity';
	const v = cleanQuantity(q);
	if (!/^\d+(\.\d+)?$/.test(v)) return `"${q}" isn't an amount`;
	if (/^[0.]+$/.test(v)) return 'the amount must be more than zero';
	const decimals = v.split('.')[1]?.length ?? 0;
	if (decimals > c.precision) return `${c.code} has ${c.precision} decimal places`;
	return null;
}

/** Problems the form can point out before asking the server. */
export function problems(d: Draft, ref: Reference): string[] {
	const out: string[] = [];
	if (!/^\d{4}-\d{2}-\d{2}$/.test(d.date)) out.push('Pick a date.');
	if (!d.description.trim()) out.push('Add a description.');
	if (!d.splits.length) out.push('Add a split.');
	d.splits.forEach((s, i) => {
		const several = d.splits.length > 1;
		const n = several ? `Split ${i + 1}: ` : '';
		const p = preview(s, ref);
		const say = (m: string) => out.push(several ? n + m : m[0].toUpperCase() + m.slice(1));
		if (!p.kind) say('choose both sides, at least one of them your account.');
		const q = quantityProblem(s.quantity, s.commodity, ref);
		if (q) say(`${q}.`);
		if (s.converted) {
			const t = quantityProblem(s.toQuantity, s.toCommodity, ref);
			if (t) say(`received: ${t}.`);
		}
		if (p.purpose && (!s.purpose || s.purpose === UNSET)) say("choose who it's for.");
		if (p.settles && s.settles && !/^\d{4}-\d{2}$/.test(s.settles))
			say('the settled month is YYYY-MM.');
	});
	return out;
}

const key = (name: string) => name.trim().toLowerCase();

/** What a side shows, typed: an account's label, a counterparty's name. */
export const UNKNOWN = 'Unknown';

/** A side as text: an account's label, a name, `UNKNOWN`, or blank if unchosen. */
export function legText(l: LegDraft, ref: Reference): string {
	if (l.account === OUTSIDE) return l.outside.trim() || UNKNOWN;
	const a = ref.accounts.get(l.account);
	return a ? accountLabel(a, ref) : '';
}

/**
 * The side `text` means: an account by its label, else someone outside by
 * name (a new counterparty if no one has it), `UNKNOWN` for unknown who,
 * blank for unchosen.
 */
export function legOf(text: string, ref: Reference): LegDraft {
	const t = key(text);
	if (!t) return { account: '', outside: '' };
	const a = [...ref.accounts.values()].find((a) => key(accountLabel(a, ref)) === t);
	if (a) return { account: a.id, outside: '' };
	const named = [...ref.counterparties.values()].some((c) => key(c.name) === t);
	if (t === key(UNKNOWN) && !named) return { account: OUTSIDE, outside: '' };
	return { account: OUTSIDE, outside: text.trim() };
}

export type SideChoice = { text: string; kind: 'account' | 'counterparty' };

/**
 * What a side's box offers for `query`: accounts (by owner), then
 * counterparties, matching anywhere in their name; `skip` (the bank's
 * account) and archived accounts left out.
 */
export function sideChoices(query: string, ref: Reference, skip?: string): SideChoice[] {
	const q = key(query);
	const hit = (t: string) => !q || t.toLowerCase().includes(q);
	const accounts: SideChoice[] = accountsByOwner(ref)
		.filter((a) => !a.archived && a.id !== skip)
		.map((a) => ({ text: accountLabel(a, ref), kind: 'account' as const }))
		.filter((c) => hit(c.text));
	const counterparties: SideChoice[] = [...ref.counterparties.values()]
		.sort((a, b) => a.name.localeCompare(b.name))
		.map((c) => ({ text: c.name, kind: 'counterparty' as const }))
		.filter((c) => hit(c.text));
	return [...accounts, ...counterparties];
}

/** Counterparty names in the draft that don't exist yet. */
export function newCounterparties(d: Pick<Draft, 'splits'>, ref: Reference): string[] {
	const known = new Set([...ref.counterparties.values()].map((c) => key(c.name)));
	const names = new Map<string, string>();
	for (const s of d.splits)
		for (const l of [s.from, s.to])
			if (l.account === OUTSIDE && l.outside.trim() && !known.has(key(l.outside)))
				// The first spelling wins.
				if (!names.has(key(l.outside))) names.set(key(l.outside), l.outside.trim());
	return [...names.values()];
}

/**
 * The request for `d`. Fields the split can't have are left out; counterparty
 * names must all exist in `ref` (create them first).
 */
export function toRequest(d: Draft, ref: Reference): Schemas['NewTransaction'] {
	const ids = new Map([...ref.counterparties.values()].map((c) => [key(c.name), c.id]));
	const side = (l: LegDraft) =>
		l.account === OUTSIDE
			? l.outside.trim()
				? { counterparty_id: ids.get(key(l.outside)) }
				: {}
			: { account_id: l.account };
	return {
		date: d.date,
		description: d.description.trim(),
		notes: d.notes.trim() || null,
		tags: d.tags
			.split(',')
			.map((t) => t.trim())
			.filter(Boolean),
		external_refs: d.externalRefs,
		splits: d.splits.map((s) => {
			const p = preview(s, ref);
			const quantity = cleanQuantity(s.quantity);
			return {
				from: { ...side(s.from), commodity_id: s.commodity, quantity },
				to: {
					...side(s.to),
					commodity_id: s.converted ? s.toCommodity : s.commodity,
					quantity: s.converted ? cleanQuantity(s.toQuantity) : quantity
				},
				category_id: p.category && s.category ? s.category : null,
				purpose_id: p.purpose && s.purpose ? s.purpose : null,
				settles: p.settles && s.settles ? s.settles : null,
				description: s.description.trim() || null
			};
		})
	};
}
