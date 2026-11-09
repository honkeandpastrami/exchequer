import { describe, expect, it } from 'vitest';
import { isEmpty, parse, queryBody, serialize, without, withFilter } from './filters';

describe('filters', () => {
	it('reads lists as repeated keys and keeps only filters', () => {
		const f = parse(
			new URLSearchParams(
				'kind=income&kind=expense&kind=bogus&category_id=none&category_id=c1&from=2026-06-01&line=claim&line=x&unpriced=true&page=2'
			)
		);
		expect(f).toEqual({
			kind: ['income', 'expense'],
			category_id: ['none', 'c1'],
			from: '2026-06-01',
			line: 'claim',
			unpriced: true
		});
	});

	it('writes what it reads', () => {
		const q = 'from=2026-06-01&settles=2026-06&party_id=p1&account_id=a1&account_id=a2';
		expect(serialize(parse(new URLSearchParams(q)))).toBe(q);
		expect(isEmpty({})).toBe(true);
		expect(isEmpty({ category_id: [] })).toBe(true);
	});

	it('removes one value, or the whole settlement filter', () => {
		expect(without({ kind: ['income', 'expense'] }, 'kind', 'income')).toEqual({
			kind: ['expense']
		});
		expect(without({ kind: ['income'] }, 'kind', 'income')).toEqual({});
		expect(
			without({ settles: '2026-06', party_id: 'p', line: 'claim', from: 'x' }, 'party_id')
		).toEqual({ from: 'x' });
		expect(withFilter({ q: 'rent' }, 'account_id', 'a1')).toEqual({
			q: 'rent',
			account_id: ['a1']
		});
		expect(withFilter({ q: 'rent' }, 'q', '')).toEqual({});
	});

	it('is the same filter as a query', () => {
		const body = queryBody(
			{ category_id: ['none', 'c1'], tag_id: ['t1', 'gone'], kind: ['expense'], q: 'rent' },
			(id) => (id === 't1' ? 'trip' : undefined)
		);
		expect(body.category_ids).toEqual([null, 'c1']);
		expect(body.tags).toEqual(['trip']);
		expect(body.kinds).toEqual(['expense']);
		expect(body.text).toBe('rent');
	});
});
