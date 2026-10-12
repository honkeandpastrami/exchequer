// The month window shared by the report pages, kept in the URL.

import { addMonths, thisMonth } from '../dates';

export type Window = { end: string; span: number; start: string };

/** From `end` and `span` in `query`; the last 12 months by default. */
export function readWindow(query: URLSearchParams): Window {
	const now = thisMonth();
	const end = query.get('end')?.match(/^\d{4}-\d{2}$/) ? query.get('end')! : now;
	const span = parseInt(query.get('span') ?? '12', 10);
	const s = span > 0 && span <= 120 ? span : 12;
	const e = end > now ? now : end;
	return { end: e, span: s, start: addMonths(e, 1 - s) };
}
