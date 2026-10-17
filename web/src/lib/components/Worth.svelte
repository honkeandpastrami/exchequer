<script lang="ts">
	// What balances in several commodities are worth in one (design §3.4):
	// the sum of those with a price, and tags for what's missing or old, so
	// a partial or stale figure is never read as a whole, current one. A
	// total only marks what's missing: its rows name it.
	import type { Worth } from '../views/accounts';
	import Amount from './Amount.svelte';

	let { worth, code, total = false }: { worth: Worth; code: string; total?: boolean } = $props();
	const priced = $derived(worth.unpriced.length === 0 || worth.value !== '0');
</script>

{#if priced}<Amount value={worth.value} {code} />{:else if total}<span class="muted">—</span>{/if}
{#if worth.stale}<span class="tag">stale price</span>{/if}
{#if worth.unpriced.length}
	{#if total}<span class="mark" aria-label="leaves out {worth.unpriced.join(', ')}: no price"
			>*</span
		>{:else}<span class="tag missing">no price: {worth.unpriced.join(', ')}</span>{/if}
{/if}

<style>
	.tag {
		display: inline-block;
		margin-left: var(--space-1);
		padding: 0 var(--space-1);
		border: var(--border-width) solid var(--color-border);
		border-radius: 999px;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		line-height: 1.4;
		white-space: nowrap;
	}
	.mark {
		margin-left: var(--space-1);
		color: var(--color-warning);
		font-weight: var(--weight-strong);
	}
	.missing {
		border-color: var(--color-warning);
		background: var(--color-warning-bg);
		color: var(--color-warning);
	}
</style>
