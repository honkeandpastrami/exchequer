// Create, read (with its ETag), update and delete for the reference data:
// one shape for every `/api/v1/<things>` and `/api/v1/<things>/{id}` pair.

import { api, done, ok, type Schemas } from './client';
import type { paths } from './schema';

type Res = { data?: unknown; error?: unknown; response: Response };
type Init = { params?: object; body?: unknown };
/** The client without per-path types: `resource` checks the paths instead. */
type Loose = Record<
	'GET' | 'POST' | 'PATCH' | 'DELETE',
	(path: string, init: Init) => Promise<Res>
>;

export type Resource<T, New, Update> = {
	create(body: New): Promise<T>;
	/** The item as stored now, and the ETag to edit it with. */
	read(id: string): Promise<{ item: T; etag: string }>;
	update(id: string, etag: string, body: Update): Promise<T>;
	remove(id: string, etag: string): Promise<void>;
};

function resource<T, New, Update, P extends keyof paths & string>(
	base: P & (`${P}/{id}` extends keyof paths ? P : never)
): Resource<T, New, Update> {
	const c = api as unknown as Loose;
	const one = `${base}/{id}`;
	const at = (id: string, etag?: string) => ({
		params: { path: { id }, ...(etag ? { header: { 'If-Match': etag } } : {}) }
	});
	return {
		create: async (body) => ok(await c.POST(base, { body })) as T,
		read: async (id) => {
			const r = await c.GET(one, at(id));
			return { item: ok(r) as T, etag: r.response.headers.get('etag') ?? '' };
		},
		update: async (id, etag, body) => ok(await c.PATCH(one, { ...at(id, etag), body })) as T,
		remove: async (id, etag) => done(await c.DELETE(one, at(id, etag)))
	};
}

export const accounts = resource<
	Schemas['Account'],
	Schemas['NewAccount'],
	Schemas['AccountUpdate'],
	'/api/v1/accounts'
>('/api/v1/accounts');
export const parties = resource<
	Schemas['Party'],
	Schemas['NewParty'],
	Schemas['PartyUpdate'],
	'/api/v1/parties'
>('/api/v1/parties');
export const commodities = resource<
	Schemas['Commodity'],
	Schemas['NewCommodity'],
	Schemas['CommodityUpdate'],
	'/api/v1/commodities'
>('/api/v1/commodities');
export const categories = resource<
	Schemas['Category'],
	Schemas['NewCategory'],
	Schemas['NewCategory'],
	'/api/v1/categories'
>('/api/v1/categories');
export const counterparties = resource<
	Schemas['Counterparty'],
	Schemas['NewCounterparty'],
	Schemas['NewCounterparty'],
	'/api/v1/counterparties'
>('/api/v1/counterparties');
export const tags = resource<Schemas['Tag'], Schemas['NewTag'], Schemas['NewTag'], '/api/v1/tags'>(
	'/api/v1/tags'
);

/** The three that are only a name. */
export type Named = Resource<
	{ id: string; name: string; system?: string | null },
	{ name: string },
	{ name: string }
>;
