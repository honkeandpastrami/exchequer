import { describe, expect, it } from 'vitest';
import { formatAmount, sign, sum } from './money';

describe('money', () => {
	it('formats without rounding', () => {
		expect(formatAmount('1234567.891')).toBe('1,234,567.891');
		expect(formatAmount('-1000.00')).toBe('−1,000.00');
		expect(formatAmount('999')).toBe('999');
		expect(formatAmount('0.10')).toBe('0.10');
	});

	it('sums exactly', () => {
		expect(sum(['0.10', '0.20'])).toBe('0.30');
		expect(sum(['1.5', '-2.25'])).toBe('-0.75');
		expect(sum([])).toBe('0');
	});

	it('signs', () => {
		expect(sign('-0.01')).toBe(-1);
		expect(sign('0.00')).toBe(0);
		expect(sign('3')).toBe(1);
	});
});
