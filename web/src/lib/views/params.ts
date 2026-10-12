// A month's settlement parameters (design §4.3, step 3): each person's
// stipend and each business's funder, pre-filled from what's in force.

import type { Schemas } from '../api/client';
import type { Reference } from '../reference';
import { cleanQuantity } from './editor';
import { SETTLEMENT_CODE, usd } from '../usd';

export type ParamsDraft = {
	/** Person ID → stipend as typed. */
	stipends: Record<string, string>;
	/** Business ID → funder ID (the household or a person). */
	funders: Record<string, string>;
};

const byName = (a: Schemas['Party'], b: Schemas['Party']) => a.name.localeCompare(b.name);

export const people = (ref: Reference) =>
	[...ref.parties.values()].filter((p) => p.kind === 'person').sort(byName);

export const businesses = (ref: Reference) =>
	[...ref.parties.values()].filter((p) => p.kind === 'business').sort(byName);

export const household = (ref: Reference) =>
	[...ref.parties.values()].find((p) => p.kind === 'household');

/** Stipends are in USD. */
export const currency = usd;

/** The parameters in force for the month; a business with none is funded by the household. */
export function draftFrom(s: Schemas['Settlement'], ref: Reference): ParamsDraft {
	const stipends = new Map(s.stipends.map((x) => [x.party_id, x.amount]));
	const funders = new Map(s.funders.map((x) => [x.business_id, x.funder_id]));
	const home = household(ref)?.id ?? '';
	return {
		stipends: Object.fromEntries(people(ref).map((p) => [p.id, stipends.get(p.id) ?? ''])),
		funders: Object.fromEntries(businesses(ref).map((b) => [b.id, funders.get(b.id) ?? home]))
	};
}

/** Problems the form can point out before asking the server. */
export function paramProblems(d: ParamsDraft, ref: Reference): string[] {
	const c = currency(ref);
	if (!c)
		return [
			`Settlement is in ${SETTLEMENT_CODE}: add the ${SETTLEMENT_CODE} commodity first (Setup → Commodities).`
		];
	const out: string[] = [];
	for (const p of people(ref)) {
		const typed = d.stipends[p.id] ?? '';
		const v = cleanQuantity(typed);
		if (!v) out.push(`Stipend for ${p.name} is missing (0 is fine).`);
		else if (!/^\d+(\.\d+)?$/.test(v))
			out.push(`Stipend for ${p.name}: "${typed}" isn't an amount.`);
		else if ((v.split('.')[1]?.length ?? 0) > c.precision)
			out.push(`Stipend for ${p.name}: ${c.code} has ${c.precision} decimal places.`);
	}
	for (const b of businesses(ref))
		if (!d.funders[b.id]) out.push(`Funder for ${b.name} is missing.`);
	return out;
}

export function toParams(d: ParamsDraft, ref: Reference): Schemas['SettlementParams'] {
	return {
		stipends: people(ref).map((p) => ({
			party_id: p.id,
			amount: cleanQuantity(d.stipends[p.id] ?? '')
		})),
		funders: businesses(ref).map((b) => ({ business_id: b.id, funder_id: d.funders[b.id] ?? '' }))
	};
}

/**
 * Whether the draft differs from what's stored for the month itself.
 * Parameters carried forward from an earlier month count as not stored:
 * closing needs them set for the month.
 */
export function unsaved(d: ParamsDraft, s: Schemas['Settlement'], ref: Reference): boolean {
	if (s.params_from !== s.month) return true;
	const stored = draftFrom(s, ref);
	const same = (a: Record<string, string>, b: Record<string, string>, num: boolean) =>
		Object.keys(a).every((k) =>
			num ? sameAmount(a[k] ?? '', b[k] ?? '') : (a[k] ?? '') === (b[k] ?? '')
		);
	return !same(d.stipends, stored.stipends, true) || !same(d.funders, stored.funders, false);
}

/** "650", "650.0" and "650.00" are the same stipend. */
function sameAmount(a: string, b: string): boolean {
	const norm = (s: string) => {
		const v = cleanQuantity(s);
		if (!/^\d+(\.\d+)?$/.test(v)) return v;
		const [i = '', f = ''] = v.split('.');
		return `${i.replace(/^0+(?=\d)/, '')}.${f.replace(/0+$/, '')}`;
	};
	return norm(a) === norm(b);
}
