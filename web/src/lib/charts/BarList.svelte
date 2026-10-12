<script lang="ts">
	// Labeled horizontal bars, longest first as given. Bar lengths are shares
	// of the largest value; the numbers shown are the exact strings.
	import Amount from '../components/Amount.svelte';
	import { largest, share, type Bar } from './scale';

	let { bars, code }: { bars: Bar[]; code?: string } = $props();

	let max = $derived(largest(bars.map((b) => b.value)));
</script>

<ul class="bars">
	{#each bars as b (b.key)}
		<li>
			<span class="label">
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- callers pass resolved links -->
				{#if b.href}<a href={b.href}>{b.label}</a>{:else}{b.label}{/if}
			</span>
			<span class="track"><span class="bar" style:width="{share(b.value, max) * 100}%"></span></span
			>
			<span class="value"><Amount value={b.value} {code} /></span>
		</li>
	{/each}
</ul>

<style>
	.bars {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: minmax(8rem, max-content) 1fr max-content;
		gap: var(--space-1) var(--space-3);
		align-items: center;
		font-size: var(--text-sm);
	}
	li {
		display: contents;
	}
	.track {
		height: 0.9rem;
		background: var(--color-surface-alt);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}
	.bar {
		display: block;
		height: 100%;
		background: var(--chart-1);
	}
	.value {
		text-align: right;
	}
</style>
