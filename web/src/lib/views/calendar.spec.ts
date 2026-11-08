import { describe, expect, it } from 'vitest';
import { beyond, cells, page, pick, title, viewOf, zoomOut } from './calendar';

describe('calendar', () => {
	it('starts weeks on Monday', () => {
		// August 2026 starts on a Saturday: five blanks, then 31 days.
		const c = cells(viewOf('2026-08-14', 'day'));
		expect(c.slice(0, 5)).toEqual([null, null, null, null, null]);
		expect(c[5]).toEqual({ value: '2026-08-01', label: '1' });
		expect(c).toHaveLength(36);
		expect(cells({ level: 'day', at: '2026-06' })[0]?.value).toBe('2026-06-01');
	});

	it('lists months and blocks of years', () => {
		expect(cells({ level: 'month', at: '2026' }).map((c) => c?.value)[7]).toBe('2026-08');
		const years = cells({ level: 'year', at: '2026' }).map((c) => c?.value);
		expect(years).toHaveLength(12);
		expect(years).toContain('2026');
		expect(cells({ level: 'year', at: '2020' })).toEqual(cells({ level: 'year', at: '2026' }));
	});

	it('pages, titles and zooms', () => {
		expect(title({ level: 'day', at: '2026-08' })).toBe('August 2026');
		expect(page({ level: 'day', at: '2026-12' }, 1).at).toBe('2027-01');
		expect(page({ level: 'year', at: '2026' }, 1).at).toBe('2038');
		expect(zoomOut({ level: 'day', at: '2026-08' })).toEqual({ level: 'month', at: '2026' });
		expect(zoomOut({ level: 'month', at: '2026' })).toEqual({ level: 'year', at: '2026' });
	});

	it('picks down to the wanted level, never past it', () => {
		expect(pick({ level: 'year', at: '2026' }, '2025', 'day')).toEqual({
			level: 'month',
			at: '2025'
		});
		expect(pick({ level: 'month', at: '2025' }, '2025-03', 'day')).toEqual({
			level: 'day',
			at: '2025-03'
		});
		expect(pick({ level: 'month', at: '2025' }, '2025-03', 'month')).toBe('2025-03');
	});

	it('disables what is past the latest allowed', () => {
		expect(beyond('2026-09', '2026-08-14')).toBe(true);
		expect(beyond('2026-08', '2026-08-14')).toBe(false);
		expect(beyond('2026-08-15', '2026-08-14')).toBe(true);
		expect(beyond('2027', '2026-08')).toBe(true);
		expect(beyond('2027', undefined)).toBe(false);
	});
});
