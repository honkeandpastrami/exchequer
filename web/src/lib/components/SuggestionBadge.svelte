<script lang="ts">
	import type { Schemas } from '../api/client';
	import { percent, sourceNames, suggesterName } from '../views/imports';

	// A suggested value's mark (docs/ai.md): the source's hue, solid when
	// confident, outlined when not. Its why is in its title, for hover, and
	// one tap away (docs/ux.md §5): with `onclick`, it's a button that opens
	// where the why is shown.
	// Compact: the percentage only, where space is short; the hue still says who.
	let {
		suggestion,
		compact = false,
		onclick
	}: {
		suggestion:
			Schemas['LabelSuggestion'] | Schemas['AccountSuggestion'] | Schemas['DecisionSuggestion'];
		compact?: boolean;
		onclick?: () => void;
	} = $props();

	const why = $derived(`${suggesterName(suggestion.suggester)} · ${suggestion.reason}`);
	const text = $derived(
		`${compact ? '' : `${sourceNames[suggestion.suggester.kind]} `}${percent(suggestion.confidence)}`
	);
</script>

{#if onclick}
	<button
		type="button"
		class="badge {suggestion.suggester.kind}"
		class:unsure={!suggestion.confident}
		title={why}
		aria-label="{text}: why"
		{onclick}>{text}</button
	>
{:else}
	<span class="badge {suggestion.suggester.kind}" class:unsure={!suggestion.confident} title={why}
		>{text}</span
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
	}
	button.badge {
		min-height: 0;
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
</style>
