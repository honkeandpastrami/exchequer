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
	transfer_duplicate: 'Transfer already recorded',
	needs_account: 'Needs account'
};

export const decisions: Record<Decision, string> = {
	new: 'Add as new',
	match: 'Already there',
	replace: 'Replace it',
	skip: 'Skip'
};

/** What a row with this status can become, its default first (the server's rules). */
export function decisionsFor(status: Status): Decision[] {
	switch (status) {
		case 'new':
			return ['new', 'skip'];
		case 'duplicate':
			return ['match', 'replace', 'skip'];
		case 'near_duplicate':
			return ['match', 'replace', 'new', 'skip'];
		case 'needs_account':
			return [];
		default:
			return ['match', 'new', 'skip'];
	}
}

/** For a transfer match, `match` and `new` mean something else. */
export function decisionLabel(status: Status, d: Decision): string {
	if (status === 'transfer_match' && d === 'match') return 'One transfer with its match';
	if (status === 'transfer_match' && d === 'new') return 'Not the same transfer';
	return decisions[d];
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
		transfer_duplicate: 0,
		needs_account: 0
	};
	for (const s of statements) for (const k of Object.keys(out) as Status[]) out[k] += s.counts[k];
	return out;
}

/** What committing would do now, by decision. */
export function plan(rows: Schemas['StagedRow'][]): Record<Decision, number> {
	const out: Record<Decision, number> = { new: 0, match: 0, replace: 0, skip: 0 };
	for (const r of rows) if (r.decision) out[r.decision] += 1;
	return out;
}

/** A row's key, unique in its import. */
export const rowKey = (r: { statement: number; position: number }) =>
	`${r.statement}/${r.position}`;

// ------------------------------------------------------- Suggestions ----
// Design §8, docs/ai.md: a label is yours, confidently suggested (committed
// as shown), unsurely suggested (shown, but not committed unless you accept
// it) or missing.

type Label = Schemas['Label'];
type Suggestion = Schemas['LabelSuggestion'];

/** In display order. */
export const labelNames: Record<Label, string> = {
	counterparty: 'Counterparty',
	category: 'Category',
	purpose: 'For'
};

/**
 * What choosing "none" means for each label. A purpose has no such choice:
 * unset, it's the account's owner, shown by name.
 */
export const noneNames: Record<Label, string> = {
	counterparty: 'Unknown',
	category: 'No category',
	purpose: "The account's owner"
};

export const sourceNames: Record<Schemas['SuggesterKind'], string> = {
	rule: 'Rule',
	local: 'Local AI',
	remote: 'Remote AI'
};

export type LabelState =
	| { kind: 'yours' }
	| { kind: 'confident'; suggestion: Suggestion }
	| { kind: 'unsure'; suggestion: Suggestion }
	| { kind: 'missing' };

export function labelState(row: Schemas['StagedRow'], label: Label): LabelState {
	if (row.chosen.includes(label)) return { kind: 'yours' };
	const suggestion = row.suggestions.find((s) => s.label === label);
	if (!suggestion) return { kind: 'missing' };
	return { kind: suggestion.confident ? 'confident' : 'unsure', suggestion };
}

/** A per-mille confidence as a whole percentage. */
export const percent = (perMille: number) => `${Math.round(perMille / 10)}%`;

/** Who made a suggestion, exactly. */
export const suggesterName = (s: Schemas['Suggester']) =>
	`${sourceNames[s.kind]}: ${s.name} ${s.version}`;

/**
 * The other side of a row, as shown: one of your accounts (a transfer), a
 * counterparty, none, or `undefined` while missing. Yours, else a
 * suggestion's.
 */
export type Side = { account: string } | { counterparty: string | null } | undefined;

export function sideOf(row: Schemas['StagedRow']): Side {
	if (row.transfer_account_id) return { account: row.transfer_account_id };
	const st = labelState(row, 'counterparty');
	if (st.kind === 'yours') return { counterparty: row.counterparty_id ?? null };
	if (st.kind === 'missing') return undefined;
	return suggestedSide(st.suggestion);
}

export function suggestedSide(s: Suggestion): Side {
	return s.transfer_account_id
		? { account: s.transfer_account_id }
		: { counterparty: s.value ?? null };
}

/** Rows commit refuses until their labels are settled. */
export const unlabeled = (rows: Schemas['StagedRow'][]) => rows.filter((r) => r.missing.length);
