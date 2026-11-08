// Import review (design §6.6-6.7): what matching found for each staged row
// (its label), what committing does with it (its choice), and what it needs
// (its state, shown as its colour).

import type { Schemas } from '../api/client';
import type { Tone } from '../components/Mark.svelte';

type Status = Schemas['RowStatus'];
type Decision = Schemas['Decision'];
type State = Schemas['ReviewState'];

/** A row's label in words. */
export const statuses: Record<Status, string> = {
	new: 'New',
	duplicate: 'Duplicate',
	possible_duplicate: 'Possible duplicate',
	transfer_pair: 'Transfer pair',
	no_account: 'No account'
};

/** A choice in words: a transfer pair's `add` is one transfer. */
export function choiceName(status: Status, d: Decision): string {
	if (d === 'add') return status === 'transfer_pair' ? 'Add as one transfer' : 'Add';
	return d === 'add_separately' ? 'Add separately' : "Don't add";
}

/** A state's mark (docs/ux.md §6), and its filter's words. */
export const states: Record<State, { tone: Tone; label: string }> = {
	needs_you: { tone: 'error', label: 'Needs you' },
	to_review: { tone: 'warning', label: 'To review' },
	ready: { tone: 'ok', label: 'Ready' },
	not_added: { tone: 'off', label: 'Not added' }
};

/** The states in display order. */
export const stateOrder: State[] = ['needs_you', 'to_review', 'ready', 'not_added'];

/** How many rows a count is, by state: "2 need you". */
const counts: Record<State, [string, string]> = {
	needs_you: ['needs you', 'need you'],
	to_review: ['to review', 'to review'],
	ready: ['ready', 'ready'],
	not_added: ['not added', 'not added']
};

/** Rows by state, those that aren't zero, in order; rows not added are
 * left out unless `all`. */
export function counted(
	rows: Schemas['StateCounts'],
	all = false
): { state: State; text: string }[] {
	return stateOrder
		.filter((s) => rows[s] > 0 && (all || s !== 'not_added'))
		.map((s) => ({ state: s, text: `${rows[s]} ${counts[s][rows[s] === 1 ? 0 : 1]}` }));
}

/** `…6789`, or what to call a statement without a number. */
export function statementName(s: Schemas['StagedStatement']): string {
	return s.number_last4 ? `…${s.number_last4}` : 'No account number';
}

const several = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** What Commit says it will do, or why it can't yet. */
export function commitLine(r: Schemas['Readiness']): { blocked: boolean; text: string } {
	if (r.no_account)
		return {
			blocked: true,
			text: several(r.no_account, 'statement needs its account', 'statements need their account')
		};
	if (r.rows.needs_you)
		return {
			blocked: true,
			text: several(r.rows.needs_you, 'row needs', 'rows need') + ' you first'
		};
	return { blocked: false, text: `adds ${several(r.adds, 'transaction', 'transactions')}` };
}

/** What committing with rows to review asks first; nothing if there are none. */
export function unreviewedQuestion(r: Schemas['Readiness']): string {
	const rows = r.rows.to_review;
	const accounts = r.accounts_to_review;
	if (!rows && !accounts) return '';
	const what = [
		rows && several(rows, 'row', 'rows'),
		accounts && several(accounts, 'statement', 'statements')
	]
		.filter(Boolean)
		.join(' and ');
	const take = rows + accounts === 1 ? 'takes' : 'take';
	return `${what} ${take} suggestions you haven't reviewed. They'll be committed as shown.`;
}

/** A row's key, unique in its import. */
export const rowKey = (r: { statement: number; position: number }) =>
	`${r.statement}/${r.position}`;

// ------------------------------------------------------- Suggestions ----
// Design §8, docs/ai.md: a label is yours, a suggestion's (committed as
// shown; its badge says how sure), or missing.

type Label = Schemas['Label'];
type Suggestion = Schemas['LabelSuggestion'];

/** In display order. */
export const labelNames: Record<Label, string> = {
	counterparty: 'To',
	category: 'Category',
	purpose: 'For'
};

export const sourceNames: Record<Schemas['SuggesterKind'], string> = {
	rule: 'Rule',
	local: 'Local AI',
	remote: 'Remote AI',
	match: 'Matching'
};

export type LabelState =
	{ kind: 'yours' } | { kind: 'suggested'; suggestion: Suggestion } | { kind: 'missing' };

export function labelState(row: Schemas['StagedRow'], label: Label): LabelState {
	if (row.chosen.includes(label)) return { kind: 'yours' };
	if (row.missing.includes(label)) return { kind: 'missing' };
	const suggestion = row.suggestions.find((s) => s.label === label);
	return suggestion ? { kind: 'suggested', suggestion } : { kind: 'missing' };
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
