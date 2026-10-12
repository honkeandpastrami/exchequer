import Big from 'big.js';
import { describe, expect, it } from 'vitest';
import { axis, labelIndexes, largest, niceStep, position, share } from './scale';

describe('scale', () => {
	it('picks round steps', () => {
		expect(niceStep(new Big(30)).toFixed()).toBe('50');
		expect(niceStep(new Big(10)).toFixed()).toBe('10');
		expect(niceStep(new Big('0.13')).toFixed()).toBe('0.2');
		expect(niceStep(new Big(1200)).toFixed()).toBe('2000');
	});

	it('covers the values and zero', () => {
		expect(axis(['-30', '120']).ticks).toEqual(['-50', '0', '50', '100', '150']);
		expect(axis(['250.75', '900.10']).ticks).toEqual(['0', '200', '400', '600', '800', '1000']);
		expect(axis(['0.3']).ticks).toEqual(['0.0', '0.1', '0.2', '0.3']);
		expect(axis([]).ticks).toEqual(['0.0', '0.2', '0.4', '0.6', '0.8', '1.0']);
	});

	it('maps values to pixels', () => {
		const a = axis(['-50', '150']);
		expect(position('-50', a, 200)).toBe(0);
		expect(position('0', a, 200)).toBe(50);
		expect(position('150', a, 200)).toBe(200);
		expect(share('25', '100')).toBe(0.25);
		expect(share('-5', '100')).toBe(0);
		expect(share('5', '0')).toBe(0);
		expect(largest(['3.50', '12', '-40'])).toBe('12');
		expect(largest([])).toBe('0');
	});

	it('thins labels, keeping the last', () => {
		expect(labelIndexes(4, 6)).toEqual([0, 1, 2, 3]);
		expect(labelIndexes(12, 6)).toEqual([1, 3, 5, 7, 9, 11]);
		expect(labelIndexes(13, 6).at(-1)).toBe(12);
	});
});
