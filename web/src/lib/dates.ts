import * as chrono from 'chrono-node';

/** Today in the browser's time zone, as `YYYY-MM-DD`. */
export function today(): string {
	return new Date().toLocaleDateString('en-CA');
}

/** The local date (browser's time zone) of a timestamp, as `YYYY-MM-DD`. */
export function localDate(timestamp: string): string {
	return new Date(timestamp).toLocaleDateString('en-CA');
}

/** This month, as `YYYY-MM`. */
export function thisMonth(): string {
	return today().slice(0, 7);
}

/** `month` (`YYYY-MM`) moved by `n` months. */
export function addMonths(month: string, n: number): string {
	const [y, m] = month.split('-').map((x) => parseInt(x, 10));
	const total = y * 12 + (m - 1) + n;
	const year = Math.floor(total / 12);
	return `${String(year).padStart(4, '0')}-${String((total % 12) + 1).padStart(2, '0')}`;
}

/** `2026-08` → `Aug 2026`. */
export function monthName(month: string): string {
	const [y, m] = month.split('-').map((x) => parseInt(x, 10));
	return new Date(Date.UTC(y, m - 1, 1)).toLocaleDateString('en-US', {
		month: 'short',
		year: 'numeric',
		timeZone: 'UTC'
	});
}

/** Every month from `from` through `to` (`YYYY-MM`), in order. */
export function monthsBetween(from: string, to: string): string[] {
	const out: string[] = [];
	for (let m = from; m <= to; m = addMonths(m, 1)) out.push(m);
	return out;
}

/** The last day of `month`, as `YYYY-MM-DD`. */
export function lastDay(month: string): string {
	const [y, m] = month.split('-').map((x) => parseInt(x, 10));
	const d = new Date(Date.UTC(y, m, 0)).getUTCDate();
	return `${month}-${String(d).padStart(2, '0')}`;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** `2026-08-14` → `Aug 14, 2026`: how every day is shown (docs/ux.md §11). */
export function dayName(day: string): string {
	const [y, m, d] = day.split('-').map((x) => parseInt(x, 10));
	return `${MONTHS[m - 1]} ${d}, ${y}`;
}

/** A timestamp's local day, shown as `Aug 14, 2026`. */
export function stampName(timestamp: string): string {
	return dayName(localDate(timestamp));
}

/** `2025-09`, `2026-08` → `Sep 2025 – Aug 2026`. */
export function rangeName(from: string, to: string): string {
	return `${monthName(from)} – ${monthName(to)}`;
}

/** `day` (`YYYY-MM-DD`) moved by `n` days. */
export function addDays(day: string, n: number): string {
	const [y, m, d] = day.split('-').map((x) => parseInt(x, 10));
	return new Date(Date.UTC(y, m - 1, d + n)).toISOString().slice(0, 10);
}

/**
 * Whatever day a person types (`8/14/2026`, `14 Aug`, `yesterday`, `last
 * Friday`), as `YYYY-MM-DD`; null when it isn't one. A day typed without a
 * year is the most recent one on or before `reference`: ledger dates are
 * almost always past.
 */
export function parseDay(text: string, reference: string = today()): string | null {
	const t = text.trim();
	const [ry, rm, rd] = reference.split('-').map((x) => parseInt(x, 10));
	const found = chrono.parse(t, new Date(ry, rm - 1, rd, 12))[0];
	// The whole text must be the date: nothing left over is ignored.
	if (!found || found.text.length !== t.length) return null;
	const s = found.start;
	let y = s.get('year')!;
	const [m, d] = [s.get('month')!, s.get('day')!];
	const iso = (y: number) =>
		`${String(y).padStart(4, '0')}-${String(m).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
	if (!s.isCertain('year') && iso(y) > reference) y -= 1;
	const day = iso(y);
	return day <= lastDay(day.slice(0, 7)) ? day : null;
}

/** What a date picker picks: a day, a month or a year. */
export type Granularity = 'day' | 'month' | 'year';

/** The current day, month or year. */
export function current(g: Granularity): string {
	return g === 'day' ? today() : g === 'month' ? thisMonth() : today().slice(0, 4);
}

/** `value` moved by `n` of its own unit. */
export function step(value: string, g: Granularity, n: number): string {
	if (g === 'day') return addDays(value, n);
	if (g === 'month') return addMonths(value, n);
	return String(parseInt(value, 10) + n).padStart(4, '0');
}

/** How a day, month or year is shown. */
export function periodName(value: string, g: Granularity): string {
	return g === 'day' ? dayName(value) : g === 'month' ? monthName(value) : value;
}
