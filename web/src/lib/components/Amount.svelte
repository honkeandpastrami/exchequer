<script lang="ts">
	import { formatAmount, sign } from '../money';

	let { value, code }: { value: string; code?: string } = $props();
</script>

<span class="amount" class:negative={sign(value) < 0}
	>{formatAmount(value)}{#if code}<span class="code">{code}</span>{/if}</span
>

<style>
	.amount {
		font-variant-numeric: var(--numeric);
		white-space: nowrap;
	}
	.negative {
		color: var(--color-negative);
	}
	.code {
		margin-left: var(--space-1);
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
</style>
