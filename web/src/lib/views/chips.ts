// The transactions page says what it shows: one chip per filter value,
// including the ones a link set that have no control on the page (a
// settlement, a purpose, a kind), each removable on its own.

import { dayName, monthName } from '../dates';
import { NONE, type Filter, type ListKey } from '../filters';
import { kinds, lineKinds } from '../labels';
import { accountLabel } from '../names';
import type { Reference } from '../reference';

export type Chip = {
	/** The filter it removes. */
	key: ListKey | 'from' | 'to' | 'q' | 'settles' | 'unpriced' | 'import_batch_id';
	/** The value of a list it removes. */
	value?: string;
	label: string;
};

const UNKNOWN = '?';

export function chips(f: Filter, ref: Reference, currency = 'USD'): Chip[] {
	const out: Chip[] = [];
	const party = (id: string) => ref.parties.get(id)?.name ?? UNKNOWN;
	const list = (key: ListKey, name: (id: string) => string) => {
		for (const value of f[key] ?? []) out.push({ key, value, label: name(value) });
	};
	if (f.q) out.push({ key: 'q', label: `“${f.q}”` });
	if (f.from) out.push({ key: 'from', label: `From ${dayName(f.from)}` });
	if (f.to) out.push({ key: 'to', label: `To ${dayName(f.to)}` });
	list('account_id', (id) => {
		const a = ref.accounts.get(id);
		return a ? accountLabel(a, ref) : UNKNOWN;
	});
	list('owner_id', (id) => `Owned by ${party(id)}`);
	list('purpose_id', (id) => (id === NONE ? 'No purpose' : `For ${party(id)}`));
	list('kind', (k) => kinds[k as keyof typeof kinds] ?? UNKNOWN);
	list('category_id', (id) =>
		id === NONE ? 'Uncategorized' : (ref.categories.get(id)?.name ?? UNKNOWN)
	);
	list('counterparty_id', (id) =>
		id === NONE ? 'No counterparty' : (ref.counterparties.get(id)?.name ?? UNKNOWN)
	);
	list('commodity_id', (id) => ref.commodities.get(id)?.code ?? UNKNOWN);
	list('tag_id', (id) => ref.tags.get(id)?.name ?? UNKNOWN);
	if (f.settles && f.party_id) {
		const line = f.line ? `, ${lineKinds[f.line].toLowerCase()}s` : '';
		out.push({
			key: 'settles',
			label: `${party(f.party_id)}’s settlement for ${monthName(f.settles)}${line}`
		});
	}
	if (f.unpriced) out.push({ key: 'unpriced', label: `No value in ${currency}` });
	if (f.import_batch_id) out.push({ key: 'import_batch_id', label: 'Import batch' });
	return out;
}
