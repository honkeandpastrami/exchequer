<script lang="ts" module>
	import { getContext } from 'svelte';
	import type { Look } from '../layout';

	const key = Symbol('section');

	/** The look of the section a component sits in; wide outside any. */
	export function look(): { readonly look: Look } {
		return getContext<{ readonly look: Look } | undefined>(key) ?? { look: 'wide' };
	}
</script>

<script lang="ts">
	// A page section with two looks (docs/ux.md §3; the rules are in
	// `layout.ts`). Its content draws the wide look until it no longer fits,
	// then the narrow one; components inside read which with `look()`, and
	// the content gets it as the snippet's argument. Sections inside it
	// decide for themselves.
	import { setContext, type Snippet } from 'svelte';
	import { next, pixels, rootFont, start, type Switch } from '../layout';

	let {
		children,
		label,
		tag = 'div'
	}: {
		children: Snippet<[Look]>;
		/** What the section is, for screen readers. */
		label?: string;
		tag?: 'div' | 'section' | 'form';
	} = $props();

	let now: Switch = $state(start);
	let node: HTMLElement | undefined = $state();

	setContext(key, {
		get look() {
			return now.look;
		}
	});

	$effect(() => {
		const el = node;
		if (!el) return;
		let frame = 0;
		const measure = () => {
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(() => {
				const margin = pixels(getComputedStyle(el).getPropertyValue('--switch-margin'), rootFont());
				const was = now;
				now = next(was, { width: el.clientWidth, content: el.scrollWidth, margin });
				// Back to wide: check that it fits now, in case the content grew.
				if (was.look !== now.look && now.look === 'wide') measure();
			});
		};
		const resized = new ResizeObserver(measure);
		resized.observe(el);
		// What it holds may change without the section changing size.
		const changed = new MutationObserver(measure);
		changed.observe(el, { childList: true, subtree: true, characterData: true });
		measure();
		return () => {
			cancelAnimationFrame(frame);
			resized.disconnect();
			changed.disconnect();
		};
	});
</script>

<svelte:element this={tag} bind:this={node} class="section" data-look={now.look} aria-label={label}
	>{@render children(now.look)}</svelte:element
>

<style>
	/* What doesn't fit stays inside, so a section's overflow never decides
	   for the section around it; focus rings still show. */
	.section {
		min-width: 0;
		overflow-x: clip;
		overflow-clip-margin: var(--space-1);
	}
</style>
