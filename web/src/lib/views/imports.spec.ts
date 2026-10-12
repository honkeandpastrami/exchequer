import { describe, expect, it } from 'vitest';
import type { Schemas } from '../api/client';
import { counted, decisionLabel, decisionsFor, plan, statementName, totals } from './imports';

const counts = (c: Partial<Schemas['RowCounts']>): Schemas['RowCounts'] => ({
	new: 0,
	duplicate: 0,
	near_duplicate: 0,
	transfer_match: 0,
	needs_account: 0,
	...c
});

const statement = (
	number_last4: string | null,
	c: Partial<Schemas['RowCounts']>
): Schemas['StagedStatement'] => ({
	position: 0,
	commodity_id: 'usd',
	number_last4,
	counts: counts(c),
	skipped: []
});

const row = (decision: Schemas['Decision'] | null): Schemas['StagedRow'] => ({
	statement: 0,
	position: 1,
	date: '2026-09-01',
	amount: '-1.00',
	description: 'x',
	status: 'new',
	decided: false,
	decision,
	raw: {}
});

describe('import review', () => {
	it('offers what the server allows, the default first', () => {
		expect(decisionsFor('new')).toEqual(['new', 'skip']);
		expect(decisionsFor('duplicate')).toEqual(['match', 'skip']);
		expect(decisionsFor('near_duplicate')).toEqual(['match', 'new', 'skip']);
		expect(decisionsFor('transfer_match')).toEqual(['match', 'new', 'skip']);
		expect(decisionsFor('needs_account')).toEqual([]);
	});

	it('calls a transfer match a transfer', () => {
		expect(decisionLabel('transfer_match', 'match')).toBe('Make it a transfer');
		expect(decisionLabel('near_duplicate', 'match')).toBe('Already there');
	});

	it('names statements by their number', () => {
		expect(statementName(statement('6789', {}))).toBe('…6789');
		expect(statementName(statement(null, {}))).toBe('No account number');
	});

	it('counts rows by status, in display order, leaving out the empty ones', () => {
		const s = [statement('1', { duplicate: 2, new: 3 }), statement('2', { new: 1 })];
		expect(counted(totals(s))).toEqual([
			['new', 4],
			['duplicate', 2]
		]);
	});

	it('plans by decision, leaving out undecided rows', () => {
		expect(plan([row('new'), row('new'), row('skip'), row(null)])).toEqual({
			new: 2,
			match: 0,
			skip: 1
		});
	});
});
