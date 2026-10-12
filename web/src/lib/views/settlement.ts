// The settlement pages (design §4.3): edges of the settlement tree month by
// month. Positive amounts go from the child to the parent.

import type { Schemas } from '../api/client';
import { partyKinds, rank } from '../labels';
import Big from 'big.js';

type Edge = Schemas['Edge'];

/**
 * Where a month stands on one edge.
 * - `open`: not closed yet.
 * - `settled`: closed, nothing outstanding.
 * - `paying-down`: closed, owing less than was carried in (a past
 *   difference being paid down).
 * - `difference`: closed, with a new difference to look at.
 */
export type Status = 'open' | 'settled' | 'paying-down' | 'difference';

export function status(edge: Edge, closed: boolean): Status {
	if (!closed) return 'open';
	const outstanding = new Big(edge.outstanding).abs();
	if (outstanding.eq(0)) return 'settled';
	return outstanding.lt(new Big(edge.carried).abs()) ? 'paying-down' : 'difference';
}

export const edgeKey = (e: Pick<Edge, 'child_id' | 'parent_id'>) => `${e.child_id}>${e.parent_id}`;

export type EdgeHistory = {
	key: string;
	child: Schemas['Party'] | undefined;
	parent: Schemas['Party'] | undefined;
	months: { month: string; closed: boolean; edge: Edge; status: Status }[];
};

/** One history per edge (people first, then businesses), months in order. */
export function byEdge(
	history: Schemas['History'],
	parties: Map<string, Schemas['Party']>
): EdgeHistory[] {
	const out = new Map<string, EdgeHistory>();
	for (const s of [...history.months].sort((a, b) => a.month.localeCompare(b.month))) {
		for (const edge of s.edges) {
			const key = edgeKey(edge);
			let h = out.get(key);
			if (!h) {
				h = {
					key,
					child: parties.get(edge.child_id),
					parent: parties.get(edge.parent_id),
					months: []
				};
				out.set(key, h);
			}
			h.months.push({ month: s.month, closed: s.closed, edge, status: status(edge, s.closed) });
		}
	}
	const kind = (p: Schemas['Party'] | undefined) => (p ? rank(partyKinds, p.kind) : 99);
	return [...out.values()].sort(
		(a, b) =>
			kind(a.child) - kind(b.child) || (a.child?.name ?? '').localeCompare(b.child?.name ?? '')
	);
}
