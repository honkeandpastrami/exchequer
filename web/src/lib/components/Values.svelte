<script lang="ts">
	// A row's values, one per line (docs/ux.md §7): `<dt>` its name, `<dd>`
	// its value, `<dd class="aside">` its badge. The badges form a column
	// right after the longest value; a value wraps, and where a badge would
	// cost it its room, the badge goes, never the text (docs/ux.md §3).
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="room"><dl class="values">{@render children()}</dl></div>

<style>
	.room {
		container-type: inline-size;
	}
	.values {
		display: grid;
		grid-template-columns: max-content minmax(0, max-content) max-content;
		justify-content: start;
		gap: 0 var(--space-3);
		align-items: baseline;
		margin: 0;
	}
	.values :global(dt) {
		color: var(--color-text-muted);
	}
	.values :global(dd) {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	@container (max-width: 16rem) {
		.values :global(.aside) {
			display: none;
		}
	}
</style>
