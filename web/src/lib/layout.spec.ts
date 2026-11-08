import { describe, expect, it } from 'vitest';
import { next, pixels, start } from './layout';

describe('next', () => {
	it('stays wide while the content fits', () => {
		expect(next(start, { width: 800, content: 800, margin: 64 })).toEqual(start);
	});

	it('ignores a rounding pixel', () => {
		expect(next(start, { width: 800, content: 800.6, margin: 64 })).toEqual(start);
	});

	it('goes narrow when the content overflows, remembering what it needed', () => {
		expect(next(start, { width: 700, content: 760, margin: 64 })).toEqual({
			look: 'narrow',
			need: 760
		});
	});

	it('stays narrow just past the switch: no flipping back and forth', () => {
		const narrow = { look: 'narrow' as const, need: 760 };
		expect(next(narrow, { width: 761, content: 761, margin: 64 })).toEqual(narrow);
		expect(next(narrow, { width: 823, content: 823, margin: 64 })).toEqual(narrow);
	});

	it('goes back to wide once there is room to spare', () => {
		const narrow = { look: 'narrow' as const, need: 760 };
		expect(next(narrow, { width: 824, content: 824, margin: 64 }).look).toBe('wide');
	});

	it('goes narrow again, needing more, when the content grew meanwhile', () => {
		const wide = next({ look: 'narrow', need: 760 }, { width: 900, content: 900, margin: 64 });
		expect(next(wide, { width: 900, content: 950, margin: 64 })).toEqual({
			look: 'narrow',
			need: 950
		});
	});
});

describe('pixels', () => {
	it('reads rem against the root font', () => {
		expect(pixels('4rem', 16)).toBe(64);
	});

	it('reads px as is', () => {
		expect(pixels(' 40px', 16)).toBe(40);
	});

	it('is zero for what it cannot read', () => {
		expect(pixels('', 16)).toBe(0);
	});
});
