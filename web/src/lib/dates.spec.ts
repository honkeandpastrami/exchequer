import { describe, expect, it } from 'vitest';
import {
	addDays,
	addMonths,
	dayName,
	lastDay,
	localDate,
	monthName,
	monthsBetween,
	parseDay,
	rangeName,
	step
} from './dates';

describe('dates', () => {
	it('moves across years', () => {
		expect(addMonths('2026-12', 1)).toBe('2027-01');
		expect(addMonths('2026-01', -1)).toBe('2025-12');
		expect(addMonths('2026-05', -17)).toBe('2024-12');
	});

	it('names months', () => {
		expect(monthName('2026-08')).toBe('Aug 2026');
	});
});

describe('month ranges', () => {
	it('lists months and last days', () => {
		expect(monthsBetween('2025-11', '2026-02')).toEqual([
			'2025-11',
			'2025-12',
			'2026-01',
			'2026-02'
		]);
		expect(monthsBetween('2026-03', '2026-02')).toEqual([]);
		expect(lastDay('2028-02')).toBe('2028-02-29');
		expect(lastDay('2026-12')).toBe('2026-12-31');
	});

	it('dates a timestamp in the local time zone', () => {
		const tz = process.env.TZ;
		process.env.TZ = 'America/New_York';
		try {
			expect(localDate('2026-03-03T03:00:00Z')).toBe('2026-03-02');
		} finally {
			if (tz === undefined) delete process.env.TZ;
			else process.env.TZ = tz;
		}
	});
});

describe('one format', () => {
	it('names days, months and ranges', () => {
		expect(dayName('2026-08-14')).toBe('Aug 14, 2026');
		expect(dayName('2026-01-05')).toBe('Jan 5, 2026');
		expect(rangeName('2025-09', '2026-08')).toBe('Sep 2025 – Aug 2026');
	});

	it('reads what is typed, either way', () => {
		expect(parseDay('2026-08-14')).toBe('2026-08-14');
		expect(parseDay(' Aug 14, 2026 ')).toBe('2026-08-14');
		expect(parseDay('aug 4 2026')).toBe('2026-08-04');
		expect(parseDay('2026-8-4')).toBe('2026-08-04');
		expect(parseDay('Feb 30, 2026')).toBeNull();
		expect(parseDay('2026-13-01')).toBeNull();
		expect(parseDay('Foo 1, 2026')).toBeNull();
		expect(parseDay('')).toBeNull();
	});

	it('steps by its own unit', () => {
		expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
		expect(addDays('2024-03-01', -1)).toBe('2024-02-29');
		expect(step('2026-08-14', 'day', -14)).toBe('2026-07-31');
		expect(step('2026-08', 'month', 12)).toBe('2027-08');
		expect(step('2026', 'year', -1)).toBe('2025');
	});
});
