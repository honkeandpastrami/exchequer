<script lang="ts">
	import type { Quantity } from '../views/transactions';
	import Amount from './Amount.svelte';

	// `inline`: in a sentence, comma-separated, instead of one per line.
	let { items, inline = false }: { items: Quantity[]; inline?: boolean } = $props();
</script>

{#each items as q, i (i)}
	{#if inline}{i ? ', ' : ''}<Amount value={q.quantity} code={q.commodity?.code ?? '?'} />{:else}
		<div><Amount value={q.quantity} code={q.commodity?.code ?? '?'} /></div>
	{/if}
{/each}
