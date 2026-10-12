/** Today in the browser's time zone, as `YYYY-MM-DD`. */
export function today(): string {
	return new Date().toLocaleDateString('en-CA');
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
