// Charts place decimal-string amounts on a pixel axis. Pixel positions are the
// only floats; every label is formatted from an exact string (design §3.5).

import Big from 'big.js';

/** One line, or one set of columns, with a value per x label. */
export type Series = { name: string; values: string[] };

/** One labeled bar; `href` is already resolved. */
export type Bar = { key: string; label: string; value: string; href?: string };

export type Axis = { lo: Big; hi: Big; ticks: string[] };

const ROUND_DOWN = 0;

function floor(x: Big): Big {
	const t = x.round(0, ROUND_DOWN);
	return t.gt(x) ? t.minus(1) : t;
}

function ceil(x: Big): Big {
	const t = x.round(0, ROUND_DOWN);
	return t.lt(x) ? t.plus(1) : t;
}

/** The smallest of 1, 2 or 5 × 10ⁿ that is at least `rough`. */
export function niceStep(rough: Big): Big {
	let p = new Big(1);
	while (p.times(10).lte(rough)) p = p.times(10);
	while (p.gt(rough)) p = p.div(10);
	for (const m of [1, 2, 5, 10]) if (p.times(m).gte(rough)) return p.times(m);
	return p.times(10);
}

/** An axis covering `values` and zero, with round tick values about `count` apart. */
export function axis(values: string[], count = 5): Axis {
	let lo = new Big(0);
	let hi = new Big(0);
	for (const v of values) {
		const b = new Big(v);
		if (b.lt(lo)) lo = b;
		if (b.gt(hi)) hi = b;
	}
	if (lo.eq(hi)) hi = lo.plus(1);
	const step = niceStep(hi.minus(lo).div(count));
	const decimals = step.gte(1) ? 0 : (step.toFixed().split('.')[1]?.length ?? 0);
	const first = floor(lo.div(step)).times(step);
	const last = ceil(hi.div(step)).times(step);
	const ticks: string[] = [];
	for (let t = first; t.lte(last); t = t.plus(step)) ticks.push(t.toFixed(decimals));
	return { lo: first, hi: last, ticks };
}

/** Where `value` falls on `a`, from 0 (its low end) to `size`. */
export function position(value: string, a: Axis, size: number): number {
	return new Big(value).minus(a.lo).div(a.hi.minus(a.lo)).times(size).toNumber();
}

/** `value` as a share of `max` (0 to 1; negatives count as 0). */
export function share(value: string, max: string): number {
	const m = new Big(max);
	if (m.lte(0)) return 0;
	const v = new Big(value);
	return v.lte(0) ? 0 : v.div(m).toNumber();
}

/** The largest of `values` (`0` for none). */
export function largest(values: string[]): string {
	return values.reduce((m, v) => (new Big(v).gt(m) ? v : m), '0');
}

/** Indexes of at most `most` evenly spaced labels among `n`, always the last. */
export function labelIndexes(n: number, most: number): number[] {
	if (n <= most) return [...Array(n).keys()];
	const every = Math.ceil(n / most);
	const out: number[] = [];
	for (let i = (n - 1) % every; i < n; i += every) out.push(i);
	return out;
}
