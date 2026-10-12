// Page state kept in the URL. With hash routing the query is part of the
// hash (`#/transactions?q=rent`), so `url.searchParams` never sees it.

/** The query in the hash of `url`. */
export function hashQuery(url: { readonly hash: string }): URLSearchParams {
	const hash = url.hash;
	const at = hash.indexOf('?');
	return new URLSearchParams(at < 0 ? '' : hash.slice(at + 1));
}

/** `query` with `key` set to `value` (removed when empty), without the `?`. */
export function withParam(query: URLSearchParams, key: string, value: string): string {
	const p = new URLSearchParams(query);
	if (value) p.set(key, value);
	else p.delete(key);
	return p.toString();
}
