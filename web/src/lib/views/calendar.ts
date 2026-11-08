// The calendar's grids (docs/ux.md §11): the days of a month, the months of
// a year, a block of years. Pure, so the component only draws them.

import { addMonths, lastDay, type Granularity } from '../dates';

/** What the calendar shows: one level of the zoom. */
export type View = { level: Granularity; at: string };

export type Cell = { value: string; label: string };

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const FULL = [
	'January',
	'February',
	'March',
	'April',
	'May',
	'June',
	'July',
	'August',
	'September',
	'October',
	'November',
	'December'
];
export const WEEKDAYS = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'];

/** Years come in blocks of twelve, aligned so each year is always in the same block. */
const blockStart = (year: number) => year - (((year % 12) + 12) % 12);

/** The view a value opens at: its own level, around the value. */
export function viewOf(value: string, level: Granularity): View {
	return { level, at: level === 'day' ? value.slice(0, 7) : value.slice(0, 4) };
}

/** The cells of a view, with null for the blanks before a month's first day. */
export function cells(v: View): (Cell | null)[] {
	if (v.level === 'day') {
		const first = new Date(`${v.at}-01T00:00:00Z`).getUTCDay();
		const blanks = (first + 6) % 7;
		const days = parseInt(lastDay(v.at).slice(8), 10);
		return [
			...Array<null>(blanks).fill(null),
			...Array.from({ length: days }, (_, i) => ({
				value: `${v.at}-${String(i + 1).padStart(2, '0')}`,
				label: String(i + 1)
			}))
		];
	}
	if (v.level === 'month') {
		return MONTHS.map((label, i) => ({
			value: `${v.at}-${String(i + 1).padStart(2, '0')}`,
			label
		}));
	}
	const start = blockStart(parseInt(v.at, 10));
	return Array.from({ length: 12 }, (_, i) => {
		const y = String(start + i).padStart(4, '0');
		return { value: y, label: y };
	});
}

/** The header of a view: `August 2026`, `2026`, `2016 – 2027`. */
export function title(v: View): string {
	if (v.level === 'day') return `${FULL[parseInt(v.at.slice(5), 10) - 1]} ${v.at.slice(0, 4)}`;
	if (v.level === 'month') return v.at;
	const start = blockStart(parseInt(v.at, 10));
	return `${start} – ${start + 11}`;
}

/** The view one page earlier or later: a month, a year, a block of years. */
export function page(v: View, n: number): View {
	if (v.level === 'day') return { ...v, at: addMonths(v.at, n) };
	const y = parseInt(v.at, 10) + n * (v.level === 'month' ? 1 : 12);
	return { ...v, at: String(y).padStart(4, '0') };
}

/** One level out: days → months → years. */
export function zoomOut(v: View): View {
	return v.level === 'day'
		? { level: 'month', at: v.at.slice(0, 4) }
		: { level: 'year', at: v.at.slice(0, 4) };
}

/** Picking a cell: the value when the view is at the wanted level, else one level in. */
export function pick(v: View, value: string, wanted: Granularity): string | View {
	if (v.level === wanted) return value;
	return v.level === 'year' ? { level: 'month', at: value } : { level: 'day', at: value };
}

/** Whether a cell is past `max` (a value at the wanted level; none when unset). */
export function beyond(value: string, max: string | undefined): boolean {
	return max !== undefined && value.slice(0, max.length) > max.slice(0, value.length);
}
