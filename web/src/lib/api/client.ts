// The typed API client. Types come from api/openapi.json (`npm run gen`).

import createClient, { type Middleware } from 'openapi-fetch';
import type { components, paths } from './schema';
import { session, signOut } from '../session.svelte';

export type Schemas = components['schemas'];
export type Problem = Schemas['Problem'];

/** Where the app is served, e.g. `https://host/prefix`: the API sits next to it. */
function baseUrl(): string {
	return new URL('.', document.baseURI).href.replace(/\/$/, '');
}

const auth: Middleware = {
	onRequest({ request }) {
		if (session.token) request.headers.set('Authorization', `Bearer ${session.token}`);
		return request;
	},
	onResponse({ response }) {
		// The token was revoked or mistyped: ask for another.
		if (response.status === 401) signOut();
		return response;
	}
};

export const api = createClient<paths>({ baseUrl: baseUrl() });
api.use(auth);

export class ApiError extends Error {
	constructor(
		readonly status: number,
		readonly problem: Problem | undefined
	) {
		super(
			problem ? `${problem.title}${problem.detail ? `: ${problem.detail}` : ''}` : `HTTP ${status}`
		);
	}
}

/** The data of a response, or an `ApiError`. */
export function ok<T>(r: { data?: T; error?: unknown; response: Response }): T {
	if (r.data === undefined) throw new ApiError(r.response.status, r.error as Problem | undefined);
	return r.data;
}

const PAGE = 500;

type Paged = { data?: { items: unknown[]; next_cursor?: string | null }; response: Response };

/** Every item of a paginated list. */
export async function all<R extends Paged>(
	page: (q: { limit: number; cursor?: string }) => Promise<R>
): Promise<NonNullable<R['data']>['items']> {
	const out: NonNullable<R['data']>['items'] = [];
	let cursor: string | undefined;
	do {
		const p = ok(await page({ limit: PAGE, cursor })) as NonNullable<R['data']>;
		out.push(...p.items);
		cursor = p.next_cursor ?? undefined;
	} while (cursor);
	return out;
}
