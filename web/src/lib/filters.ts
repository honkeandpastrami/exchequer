// The transactions filter (design §7): what a report or a statement computed
// a figure from, and so what the list behind that figure shows. The server
// takes the same filter on `/query` and on the list; every link to the list
// is built here, so a figure and its list can't disagree.

import type { Schemas } from './api/client';

type Shared = Schemas['TransactionFilter'];

/** The shared filter, plus what only the list takes. */
export type Filter = { [K in keyof Shared]?: NonNullable<Shared[K]> } & {
	import_batch_id?: string;
	/** These transactions only; never in a link. */
	id?: string[];
};

/** A filter a report returned. */
export function fromReport(f: Shared): Filter {
	return Object.fromEntries(Object.entries(f).filter(([, v]) => v != null)) as Filter;
}

/** The filters that take several values (repeated keys); `none` is "no value". */
export const LISTS = [
	'account_id',
	'owner_id',
	'purpose_id',
	'category_id',
	'counterparty_id',
	'kind',
	'commodity_id',
	'tag_id'
] as const;
export type ListKey = (typeof LISTS)[number];

const SINGLES = ['from', 'to', 'q', 'settles', 'party_id', 'line', 'import_batch_id'] as const;
type SingleKey = (typeof SINGLES)[number];

/** The value meaning "no value there" (no category, no purpose). */
export const NONE = 'none';

const KINDS: readonly string[] = [
	'expense',
	'income',
	'transfer',
	'conversion',
	'adjustment'
] satisfies Schemas['KindName'][];
const LINES: readonly string[] = ['claim', 'payment'] satisfies Schemas['LineKind'][];

/** The filter in a URL's query; what isn't a filter is ignored. */
export function parse(query: URLSearchParams): Filter {
	const f: Filter = {};
	for (const key of LISTS) {
		const values = [...new Set(query.getAll(key).filter((v) => v))];
		if (key === 'kind') {
			const kinds = values.filter((v) => KINDS.includes(v)) as Schemas['KindName'][];
			if (kinds.length) f.kind = kinds;
		} else if (values.length) f[key] = values;
	}
	for (const key of SINGLES) {
		const v = query.get(key);
		if (!v) continue;
		if (key === 'line') {
			if (LINES.includes(v)) f.line = v as Schemas['LineKind'];
		} else f[key] = v;
	}
	if (query.get('unpriced') === 'true') f.unpriced = true;
	return f;
}

/** The filter as a query string, without the `?`: lists as repeated keys. */
export function serialize(f: Filter): string {
	const q = new URLSearchParams();
	for (const key of SINGLES) {
		const v = f[key];
		if (v) q.set(key, v);
	}
	for (const key of LISTS) for (const v of f[key] ?? []) q.append(key, v);
	if (f.unpriced) q.set('unpriced', 'true');
	return q.toString();
}

/** Whether the filter takes everything. */
export function isEmpty(f: Filter): boolean {
	return serialize(f) === '';
}

/** The path of the list of transactions behind a figure computed with `f`. */
export function transactionsPath(f: Filter): '/transactions' | `/transactions?${string}` {
	const q = serialize(f);
	return q ? `/transactions?${q}` : '/transactions';
}

/** `f` without one value of a list, or without a single filter. */
export function without(f: Filter, key: ListKey | SingleKey | 'unpriced', value?: string): Filter {
	const out: Filter = { ...f };
	if ((LISTS as readonly string[]).includes(key)) {
		const k = key as ListKey;
		const rest = (f[k] ?? []).filter((v) => v !== value);
		if (rest.length) (out as Record<ListKey, string[]>)[k] = rest;
		else delete out[k];
	} else {
		delete out[key as SingleKey | 'unpriced'];
	}
	// The settlement filter is one thing: its parts go together.
	if (key === 'settles' || key === 'party_id') {
		delete out.settles;
		delete out.party_id;
		delete out.line;
	}
	return out;
}

/** `f` with one list or single filter replaced (removed when empty). */
export function withFilter(
	f: Filter,
	key: ListKey | SingleKey,
	value: string | string[] | undefined
): Filter {
	const out: Filter = { ...f };
	const empty = value === undefined || value.length === 0;
	if (empty) delete out[key];
	else if ((LISTS as readonly string[]).includes(key))
		(out as Record<string, string[]>)[key] = Array.isArray(value) ? value : [value];
	else (out as Record<string, string>)[key] = Array.isArray(value) ? value[0] : value;
	return out;
}

const ids = (values: string[] | undefined) => (values ?? []).map((v) => (v === NONE ? null : v));

/**
 * The same filter as a `/query` body. Tags are by name there; `tagName` looks
 * them up. The settlement filter has no counterpart in a query.
 */
export function queryBody(
	f: Filter,
	tagName: (id: string) => string | undefined
): Schemas['QueryRequest'] {
	return {
		from: f.from ?? undefined,
		to: f.to ?? undefined,
		account_ids: f.account_id ?? [],
		owner_ids: f.owner_id ?? [],
		purpose_ids: ids(f.purpose_id),
		category_ids: ids(f.category_id),
		counterparty_ids: ids(f.counterparty_id),
		kinds: f.kind ?? [],
		commodity_ids: f.commodity_id ?? [],
		tags: (f.tag_id ?? []).flatMap((id) => tagName(id) ?? []),
		text: f.q ?? undefined
	};
}
