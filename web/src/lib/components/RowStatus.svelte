<script lang="ts">
	// A row's status as one sentence (docs/ux.md §5): its mark and words, then
	// what it's about ("Duplicate of" a transaction, "Transfer pair with" an
	// account and row). What follows the words wraps under itself, never
	// under the mark.
	import type { Snippet } from 'svelte';
	import Mark, { type Tone } from './Mark.svelte';

	let {
		tone,
		state,
		words,
		children
	}: {
		tone: Tone;
		/** The row's state, for screen readers: the mark says it by colour. */
		state: string;
		/** The status: "Duplicate of", "New". */
		words: string;
		/** What it's about, if anything. */
		children?: Snippet;
	} = $props();
</script>

<p class="said">
	<span class="words"><Mark {tone} label={state} quiet /> {words}</span>
	{#if children}<span class="about">{@render children()}</span>{/if}
</p>

<style>
	.said {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: 0 var(--space-1);
		margin: 0;
	}
	.words {
		font-weight: var(--weight-strong);
	}
	.about {
		overflow-wrap: anywhere;
	}
</style>
