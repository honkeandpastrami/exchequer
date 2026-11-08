// The layout switch (docs/ux.md §3): a page section has a wide look and a
// narrow one, and switches as a whole. It goes narrow when its wide look
// stops fitting, judged by what it holds, and back to wide only once there's
// room to spare, so it never flips back and forth near the switch.

export type Look = 'wide' | 'narrow';

export type Switch = {
	look: Look;
	/** The width the wide look needed when it last stopped fitting. */
	need: number;
};

export type Measure = {
	/** The section's width. */
	width: number;
	/** The width its content takes; more than `width` when it doesn't fit. */
	content: number;
	/** The room to spare before going back to wide. */
	margin: number;
};

export const start: Switch = { look: 'wide', need: 0 };

export function next(s: Switch, m: Measure): Switch {
	if (s.look === 'wide') {
		// Less than a pixel is rounding, not overflow.
		return m.content > m.width + 1 ? { look: 'narrow', need: m.content } : s;
	}
	return m.width >= s.need + m.margin ? { look: 'wide', need: s.need } : s;
}

/* eslint-disable no-restricted-globals -- lengths in pixels, not amounts */

/** A length token (`3rem`, `40px`) in pixels. */
export function pixels(token: string, rootFont: number): number {
	const n = parseFloat(token);
	if (Number.isNaN(n)) return 0;
	return token.trim().endsWith('rem') ? n * rootFont : n;
}

/** The page's font size in pixels: what a rem is. */
export function rootFont(): number {
	return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
}
