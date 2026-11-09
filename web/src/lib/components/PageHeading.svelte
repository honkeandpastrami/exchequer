<script lang="ts">
	// A page's title and the controls beside it (docs/ux.md §3): one row while
	// they fit, one under the other once they don't.
	import type { Snippet } from 'svelte';
	import Section from './Section.svelte';

	let { children: inner }: { children: Snippet } = $props();
</script>

<Section label="Heading">
	{#snippet children(look)}
		<div class="heading" class:narrow={look === 'narrow'}>{@render inner()}</div>
	{/snippet}
</Section>

<style>
	.heading {
		display: flex;
		align-items: baseline;
		gap: var(--space-5);
		margin-bottom: var(--space-3);
	}
	.heading > :global(*) {
		flex: none;
	}
	.heading.narrow {
		flex-direction: column;
		align-items: start;
		gap: var(--space-2);
	}
	.heading :global(h1) {
		margin: 0;
	}
</style>
