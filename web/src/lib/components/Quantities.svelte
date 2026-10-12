<script lang="ts">
	import type { Quantity } from '../views/transactions';
	import Amount from './Amount.svelte';

	let { items }: { items: Quantity[] } = $props();
</script>

{#each items as q, i (i)}
	<div><Amount value={q.quantity} code={q.commodity?.code ?? '?'} /></div>
{/each}
