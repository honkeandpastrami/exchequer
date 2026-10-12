import { describe, expect, it } from 'vitest';
import { addMonths, monthName } from './dates';

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
