import { describe, expect, it } from 'vitest';
import { addMonths, lastDay, localDate, monthName, monthsBetween } from './dates';

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
