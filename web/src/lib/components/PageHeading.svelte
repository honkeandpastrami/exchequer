<script lang="ts">
	// A page's title and the controls beside it, on one row while they fit.
	import type { Snippet } from 'svelte';
	import OneRow from './OneRow.svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="heading">
	<OneRow label="Heading">{@render children()}</OneRow>
</div>

<style>
	.heading :global(h1) {
		margin: 0;
	}
</style>
