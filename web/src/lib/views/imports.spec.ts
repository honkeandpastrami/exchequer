import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import {
	choiceName,
	commitLine,
	counted,
	labelState,
	percent,
	sideOf,
	statementName,
	unreviewedQuestion
} from './imports';

const statement = (number_last4: string | null): Schemas['StagedStatement'] => ({
	position: 0,
	commodity_id: 'usd',
	number_last4,
	rows: { needs_you: 0, to_review: 0, ready: 0, not_added: 0 },
	skipped: [],
	account_suggested: false,
	reviewed: false,
	state: 'ready'
});

const row = (more: Partial<Schemas['StagedRow']> = {}): Schemas['StagedRow'] => ({
	statement: 0,
	position: 1,
	date: '2026-09-01',
	amount: '-1.00',
	description: 'x',
	status: 'new',
	options: ['add', 'dont_add'],
	decided: false,
	reviewed: false,
	state: 'ready',
	decision: 'add',
	tags: [],
	raw: {},
	chosen: [],
	suggestions: [],
	missing: [],
	not_applicable: [],
	splits: [],
	...more
});

const suggestion = (label: Schemas['Label'], confidence: number): Schemas['LabelSuggestion'] => ({
	label,
	value: null,
	confidence,
	confident: confidence >= 900,
	reason: 'like before',
	suggester: { id: 's', kind: 'local', name: 'lexical-knn', version: '1', details: {} }
});

const readiness = (more: Partial<Schemas['Readiness']>): Schemas['Readiness'] => ({
	state: 'ready',
	rows: { needs_you: 0, to_review: 0, ready: 0, not_added: 0 },
	no_account: 0,
	accounts_to_review: 0,
	adds: 0,
	...more
});

describe('import review', () => {
	it('says what each choice adds', () => {
		expect(choiceName('new', 'add')).toBe('Add');
		expect(choiceName('duplicate', 'dont_add')).toBe("Don't add");
		expect(choiceName('transfer_pair', 'add')).toBe('Add as one transfer');
		expect(choiceName('transfer_pair', 'add_separately')).toBe('Add separately');
	});

	it('names statements by their number', () => {
		expect(statementName(statement('6789'))).toBe('…6789');
		expect(statementName(statement(null))).toBe('No account number');
	});

	it('counts rows by state, in order, leaving out empty ones and rows not added', () => {
		const rows = { needs_you: 2, to_review: 1, ready: 0, not_added: 3 };
		expect(counted(rows).map((c) => c.text)).toEqual(['2 need you', '1 to review']);
		expect(counted(rows, true).map((c) => c.text)).toEqual([
			'2 need you',
			'1 to review',
			'3 not added'
		]);
		expect(counted({ ...rows, needs_you: 1 })[0].text).toBe('1 needs you');
	});

	it('says what Commit does, or why it can’t', () => {
		expect(commitLine(readiness({ no_account: 1 }))).toEqual({
			blocked: true,
			text: '1 statement needs its account'
		});
		expect(
			commitLine(readiness({ rows: { needs_you: 2, to_review: 0, ready: 0, not_added: 0 } }))
		).toEqual({ blocked: true, text: '2 rows need you first' });
		expect(commitLine(readiness({ adds: 39 }))).toEqual({
			blocked: false,
			text: 'adds 39 transactions'
		});
	});

	it('asks before committing what you haven’t reviewed', () => {
		expect(unreviewedQuestion(readiness({}))).toBe('');
		expect(
			unreviewedQuestion(
				readiness({
					rows: { needs_you: 0, to_review: 4, ready: 0, not_added: 0 },
					accounts_to_review: 1
				})
			)
		).toBe(
			"4 rows and 1 statement take suggestions you haven't reviewed. They'll be committed as shown."
		);
	});

	it('tells your labels from suggested and missing ones', () => {
		const r = row({
			chosen: ['purpose'],
			suggestions: [suggestion('counterparty', 930), suggestion('category', 610)],
			missing: []
		});
		expect(labelState(r, 'purpose').kind).toBe('yours');
		expect(labelState(r, 'counterparty').kind).toBe('suggested');
		expect(labelState(r, 'category').kind).toBe('suggested');
		expect(labelState({ ...r, suggestions: [] }, 'counterparty').kind).toBe('missing');
		expect(labelState({ ...r, missing: ['category'] }, 'category').kind).toBe('missing');
		expect(percent(926)).toBe('93%');
	});
});

describe('the other side', () => {
	it('is an account for a transfer, else a counterparty, yours or suggested', () => {
		const r = row();
		expect(sideOf(r)).toBeUndefined();
		expect(sideOf({ ...r, transfer_account_id: 'card' })).toEqual({ account: 'card' });
		expect(sideOf({ ...r, chosen: ['counterparty'] })).toEqual({ counterparty: null });
		const suggested = (s: Partial<Schemas['LabelSuggestion']>) =>
			sideOf({
				...r,
				suggestions: [
					{
						label: 'counterparty',
						confidence: 950,
						confident: true,
						reason: 'x',
						suggester: { id: 's', kind: 'rule', name: 'rules', version: '1', details: {} },
						...s
					}
				]
			});
		expect(suggested({ value: 'store' })).toEqual({ counterparty: 'store' });
		expect(suggested({ transfer_account_id: 'card' })).toEqual({ account: 'card' });
	});
});
