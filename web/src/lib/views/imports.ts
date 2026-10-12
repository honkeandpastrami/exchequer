// Import review (design §6.6-6.7): what each staged row is and what
// committing will do with it.

import type { Schemas } from '../api/client';

type Status = Schemas['RowStatus'];
type Decision = Schemas['Decision'];

/** In display order. */
export const statuses: Record<Status, string> = {
	new: 'New',
	duplicate: 'Duplicate',
	near_duplicate: 'Near duplicate',
	transfer_match: 'Transfer match',
	needs_account: 'Needs account'
};

export const decisions: Record<Decision, string> = {
	new: 'Add as new',
	match: 'Already there',
	skip: 'Skip'
};

/** What a row with this status can become, its default first (the server's rules). */
export function decisionsFor(status: Status): Decision[] {
	switch (status) {
		case 'new':
			return ['new', 'skip'];
		case 'duplicate':
			return ['match', 'skip'];
		case 'needs_account':
			return [];
		default:
			return ['match', 'new', 'skip'];
	}
}

/** For a transfer match, `match` means something else. */
export function decisionLabel(status: Status, d: Decision): string {
	return status === 'transfer_match' && d === 'match' ? 'Make it a transfer' : decisions[d];
}

/** `…6789`, or what to call a statement without a number. */
export function statementName(s: Schemas['StagedStatement']): string {
	return s.number_last4 ? `…${s.number_last4}` : 'No account number';
}

/** The statuses with rows, in display order, with how many. */
export function counted(counts: Schemas['RowCounts']): [Status, number][] {
	return (Object.keys(statuses) as Status[])
		.map((s): [Status, number] => [s, counts[s]])
		.filter(([, n]) => n > 0);
}

/** Every statement's counts added up. */
export function totals(statements: Schemas['StagedStatement'][]): Schemas['RowCounts'] {
	const out: Schemas['RowCounts'] = {
		new: 0,
		duplicate: 0,
		near_duplicate: 0,
		transfer_match: 0,
		needs_account: 0
	};
	for (const s of statements) for (const k of Object.keys(out) as Status[]) out[k] += s.counts[k];
	return out;
}

/** What committing would do now, by decision. */
export function plan(rows: Schemas['StagedRow'][]): Record<Decision, number> {
	const out: Record<Decision, number> = { new: 0, match: 0, skip: 0 };
	for (const r of rows) if (r.decision) out[r.decision] += 1;
	return out;
}

/** A row's key, unique in its import. */
export const rowKey = (r: { statement: number; position: number }) =>
	`${r.statement}/${r.position}`;
