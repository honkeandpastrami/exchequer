// The report pages: query rows turned into chart series and ranked lists.

import type { Schemas } from '../api/client';
import type { Series } from '../charts/scale';
import { minus, negate, sign, sum, zero } from '../money';

type Row = Schemas['Row'];

/** Zero at the scale of `amount` (`12.50` → `0.00`). */
const zeroLike = (amount: string) => zero(amount.split('.')[1]?.length ?? 0);

/** The commodities that appear in `rows`, `preferred` first when present. */
export function commoditiesIn(rows: Row[], preferred?: string | null): string[] {
	const ids = [...new Set(rows.map((r) => r.commodity_id))];
	return preferred && ids.includes(preferred)
		? [preferred, ...ids.filter((i) => i !== preferred)]
		: ids;
}

/** The distinct buckets in `rows`, oldest first. */
export function buckets(rows: Row[]): string[] {
	return [...new Set(rows.flatMap((r) => (r.bucket ? [r.bucket] : [])))].sort();
}

/**
 * One series per account or owner, in one commodity, valued at each bucket
 * (zero where a row is missing), sorted by name. With more than one series a
 * `Total` comes last.
 */
export function balanceSeries(
	rows: Row[],
	by: 'account_id' | 'owner_id',
	commodity: string,
	names: Map<string, string>,
	at: string[]
): Series[] {
	const values = new Map<string, Map<string, string>>();
	let none = '0';
	for (const r of rows) {
		if (r.commodity_id !== commodity || !r.bucket) continue;
		none = zeroLike(r.amount);
		const key = r[by] ?? '';
		if (!values.has(key)) values.set(key, new Map());
		values.get(key)!.set(r.bucket, r.amount);
	}
	const series = [...values.entries()]
		.map(([key, v]) => ({
			name: names.get(key) ?? '(none)',
			values: at.map((b) => v.get(b) ?? none)
		}))
		.sort((a, b) => a.name.localeCompare(b.name));
	if (series.length > 1) {
		series.push({
			name: 'Total',
			values: at.map((_, i) => sum(series.map((s) => s.values[i])))
		});
	}
	return series;
}

export type CategoryTotal = { category: string | null; total: string; monthly: string[] };

/**
 * Flow rows grouped by category and bucketed by month, in one commodity, as
 * totals per category (largest first) and per month. `spending` flips the
 * sign so money out reads positive.
 */
export function byCategory(
	rows: Row[],
	commodity: string,
	months: string[],
	spending: boolean
): { categories: CategoryTotal[]; monthly: string[]; total: string } {
	const flip = (v: string) => (spending ? negate(v) : v);
	const cells = new Map<string | null, Map<string, string>>();
	let none = '0';
	for (const r of rows) {
		if (r.commodity_id !== commodity || !r.bucket) continue;
		none = zeroLike(r.amount);
		const c = r.category_id ?? null;
		if (!cells.has(c)) cells.set(c, new Map());
		const month = r.bucket.slice(0, 7);
		const m = cells.get(c)!;
		m.set(month, sum([m.get(month) ?? '0', flip(r.amount)]));
	}
	const categories = [...cells.entries()]
		.map(([category, m]) => {
			const monthly = months.map((mo) => m.get(mo) ?? none);
			return { category, monthly, total: sum(monthly) };
		})
		.filter((c) => sign(c.total) !== 0 || c.monthly.some((v) => sign(v) !== 0))
		.sort((a, b) => sign(minus(b.total, a.total)));
	const monthly = months.map((_, i) => sum([none, ...categories.map((c) => c.monthly[i])]));
	return { categories, monthly, total: sum([none, ...monthly]) };
}
