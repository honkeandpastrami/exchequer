<script lang="ts">
	import { tick } from 'svelte';
	import type { Schemas } from '../api/client';
	import { pixels, rootFont } from '../layout';
	import { percent, sourceNames, suggesterName } from '../views/imports';

	// A suggested value's mark (docs/ai.md): the source's hue, solid when
	// confident, outlined when not. Its why shows in a bubble next to it, on
	// hover from whatever can hover (a mouse, a pencil) and on a tap
	// (docs/ux.md §6); the bubble floats, so nothing under it moves.
	// Compact: the percentage only, where space is short; the hue still says who.
	let {
		suggestion,
		compact = false
	}: {
		suggestion:
			Schemas['LabelSuggestion'] | Schemas['AccountSuggestion'] | Schemas['DecisionSuggestion'];
		compact?: boolean;
	} = $props();

	const why = $derived(`${suggesterName(suggestion.suggester)}: ${suggestion.reason}`);
	const text = $derived(
		`${compact ? '' : `${sourceNames[suggestion.suggester.kind]} `}${percent(suggestion.confidence)}`
	);

	const uid = $props.id();
	let badge: HTMLButtonElement | undefined = $state();
	let bubble: HTMLElement | undefined = $state();
	/** How it was opened: a hover closes when the pointer leaves, a tap when
	 * something else is tapped. */
	let shown: 'hover' | 'tap' | null = $state(null);

	async function show(how: 'hover' | 'tap') {
		shown = how;
		await tick();
		if (!badge || !bubble) return;
		bubble.showPopover();
		// Under the badge, kept inside the window.
		const at = badge.getBoundingClientRect();
		const gutter = pixels(getComputedStyle(bubble).getPropertyValue('--gutter'), rootFont());
		const room = document.documentElement.clientWidth - gutter;
		bubble.style.top = `${at.bottom + 4}px`;
		bubble.style.left = `${Math.max(gutter, Math.min(at.left, room - bubble.offsetWidth))}px`;
	}
	function hide() {
		if (bubble?.matches(':popover-open')) bubble.hidePopover();
		shown = null;
	}

	$effect(() => {
		if (!shown) return;
		const away = (e: PointerEvent) => {
			if (shown === 'tap' && e.target !== badge) hide();
		};
		const key = (e: KeyboardEvent) => {
			if (e.key === 'Escape') hide();
		};
		// It sits where the badge was: scrolling would leave it behind.
		window.addEventListener('pointerdown', away);
		window.addEventListener('keydown', key);
		window.addEventListener('scroll', hide, { capture: true, passive: true });
		window.addEventListener('resize', hide);
		return () => {
			window.removeEventListener('pointerdown', away);
			window.removeEventListener('keydown', key);
			window.removeEventListener('scroll', hide, { capture: true });
			window.removeEventListener('resize', hide);
		};
	});
</script>

<button
	bind:this={badge}
	type="button"
	class="badge {suggestion.suggester.kind}"
	class:unsure={!suggestion.confident}
	aria-label="{text}: why"
	aria-describedby={shown ? `${uid}-why` : undefined}
	aria-expanded={!!shown}
	onpointerenter={(e) => e.pointerType !== 'touch' && !shown && show('hover')}
	onpointerleave={() => shown === 'hover' && hide()}
	onclick={() => (shown === 'tap' ? hide() : show('tap'))}>{text}</button
>
{#if shown}
	<span bind:this={bubble} id="{uid}-why" class="bubble" role="tooltip" popover="manual">{why}</span
	>
{/if}

<style>
	.badge {
		--hue: var(--color-suggest-local);
		display: inline-block;
		padding: 0 var(--space-1);
		border: var(--border-width) solid var(--hue);
		border-radius: var(--radius-sm);
		background: var(--hue);
		color: var(--color-on-suggest);
		font-size: var(--text-xs);
		line-height: var(--leading);
		white-space: nowrap;
		vertical-align: baseline;
		cursor: pointer;
	}
	.rule {
		--hue: var(--color-suggest-rule);
	}
	.remote {
		--hue: var(--color-suggest-remote);
	}
	.match {
		--hue: var(--color-suggest-match);
	}
	.unsure {
		background: transparent;
		color: var(--hue);
		border-style: dashed;
	}
	/* In the top layer, placed by `show`: nothing it covers moves. */
	.bubble {
		inset: auto;
		margin: 0;
		max-width: min(22rem, calc(100vw - 2 * var(--gutter)));
		padding: var(--space-2) var(--space-3);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		background: var(--color-surface);
		color: var(--color-text);
		box-shadow: var(--shadow-1);
		font: var(--text-md) / var(--leading) var(--font-ui);
		overflow-wrap: anywhere;
	}
</style>
