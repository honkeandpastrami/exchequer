// Names for the IDs in API responses: parties, commodities, accounts,
// counterparties, categories and tags, loaded once per page load.

import { all, api, type Schemas } from './api/client';

export type Reference = {
	parties: Map<string, Schemas['Party']>;
	commodities: Map<string, Schemas['Commodity']>;
	accounts: Map<string, Schemas['Account']>;
	counterparties: Map<string, Schemas['Counterparty']>;
	categories: Map<string, Schemas['Category']>;
	tags: Map<string, Schemas['Tag']>;
};

const byId = <T extends { id: string }>(items: T[]) => new Map(items.map((i) => [i.id, i]));

let cached: Promise<Reference> | undefined;

export function reference(): Promise<Reference> {
	cached ??= load().catch((e) => {
		cached = undefined;
		throw e;
	});
	return cached;
}

/** After an edit to any of them. */
export function forgetReference() {
	cached = undefined;
}

async function load(): Promise<Reference> {
	const [parties, commodities, accounts, counterparties, categories, tags] = await Promise.all([
		all((query) => api.GET('/api/v1/parties', { params: { query } })),
		all((query) => api.GET('/api/v1/commodities', { params: { query } })),
		all((query) => api.GET('/api/v1/accounts', { params: { query } })),
		all((query) => api.GET('/api/v1/counterparties', { params: { query } })),
		all((query) => api.GET('/api/v1/categories', { params: { query } })),
		all((query) => api.GET('/api/v1/tags', { params: { query } }))
	]);
	return {
		parties: byId(parties),
		commodities: byId(commodities),
		accounts: byId(accounts),
		counterparties: byId(counterparties),
		categories: byId(categories),
		tags: byId(tags)
	};
}

/** Sorted by name, for pickers. */
export function sorted<T extends { name: string }>(m: Map<string, T>): T[] {
	return [...m.values()].sort((a, b) => a.name.localeCompare(b.name));
}
