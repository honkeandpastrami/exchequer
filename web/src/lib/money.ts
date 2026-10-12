// Amounts arrive as decimal strings and stay exact: never parsed into floats.

import Big from 'big.js';

const MINUS = '−';

/** `-1234.5` → `−1,234.5`: thousands grouped, decimals kept as given. */
export function formatAmount(value: string): string {
	const negative = value.startsWith('-');
	const [int, frac] = (negative ? value.slice(1) : value).split('.');
	const grouped = int.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
	return (negative ? MINUS : '') + grouped + (frac !== undefined ? `.${frac}` : '');
}

/** The exact sum, at the largest scale among the inputs. */
export function sum(values: string[]): string {
	const scale = Math.max(0, ...values.map((v) => v.split('.')[1]?.length ?? 0));
	return values.reduce((a, v) => a.plus(v), new Big(0)).toFixed(scale);
}

export function sign(value: string): -1 | 0 | 1 {
	return new Big(value).cmp(0) as -1 | 0 | 1;
}

/** `a − b`, exactly, at the larger scale of the two. */
export function minus(a: string, b: string): string {
	const scale = Math.max(...[a, b].map((v) => v.split('.')[1]?.length ?? 0));
	return new Big(a).minus(b).toFixed(scale);
}
