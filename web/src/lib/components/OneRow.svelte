<script lang="ts">
	// Items side by side while they fit, one per line once they don't
	// (docs/ux.md §3): a page's heading, a row of controls. The row switches
	// as a whole, never wrapping one item at a time.
	import type { Snippet } from 'svelte';
	import Section from './Section.svelte';

	let { children: inner, label }: { children: Snippet; label: string } = $props();
</script>

<Section {label}>
	{#snippet children(look)}
		<div class="row" class:narrow={look === 'narrow'}>{@render inner()}</div>
	{/snippet}
</Section>

<style>
	.row {
		display: flex;
		align-items: baseline;
		gap: var(--space-3) var(--space-5);
		margin-bottom: var(--space-4);
	}
	.row > :global(*) {
		flex: none;
	}
	.row.narrow {
		flex-direction: column;
		align-items: start;
		gap: var(--space-2);
	}
</style>
