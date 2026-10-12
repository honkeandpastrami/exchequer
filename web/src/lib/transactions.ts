// Transactions fetched by ID, a few at a time, for pages that only have
// pointers to them (settlement lines).

import { api, ok, type Schemas } from './api/client';

const AT_ONCE = 6;

export async function transactionsById(
	ids: Iterable<string>
): Promise<Map<string, Schemas['Transaction']>> {
	const queue = [...new Set(ids)];
	const out = new Map<string, Schemas['Transaction']>();
	async function worker() {
		for (let id = queue.pop(); id; id = queue.pop()) {
			const tx = ok(await api.GET('/api/v1/transactions/{id}', { params: { path: { id } } }));
			out.set(id, tx);
		}
	}
	await Promise.all(Array.from({ length: AT_ONCE }, worker));
	return out;
}
