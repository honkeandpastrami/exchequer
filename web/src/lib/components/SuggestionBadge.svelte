<script lang="ts">
	import type { Schemas } from '../api/client';
	import { percent, sourceNames, suggesterName } from '../views/imports';

	// A suggested value's mark (docs/ai.md): the source's hue, solid when
	// confident, outlined when not.
	let { suggestion }: { suggestion: Schemas['LabelSuggestion'] | Schemas['AccountSuggestion'] } =
		$props();
</script>

<span
	class="badge {suggestion.suggester.kind}"
	class:unsure={!suggestion.confident}
	title="{suggesterName(suggestion.suggester)} · {suggestion.reason}"
	>{sourceNames[suggestion.suggester.kind]} {percent(suggestion.confidence)}</span
>

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
		white-space: nowrap;
		vertical-align: baseline;
	}
	.rule {
		--hue: var(--color-suggest-rule);
	}
	.remote {
		--hue: var(--color-suggest-remote);
	}
	.unsure {
		background: transparent;
		color: var(--hue);
		border-style: dashed;
	}
</style>
