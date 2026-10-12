<script lang="ts">
	// A run of whole months ending at `end`, moved a window at a time.
	import { addMonths, monthName, thisMonth } from '../dates';

	let {
		end,
		span,
		onchange
	}: { end: string; span: number; onchange: (end: string, span: number) => void } = $props();

	const spans = [3, 6, 12, 24, 60];
	let start = $derived(addMonths(end, 1 - span));
</script>

<span class="window">
	<button class="quiet" onclick={() => onchange(addMonths(end, -span), span)} aria-label="Earlier"
		>←</button
	>
	<strong>{monthName(start)} – {monthName(end)}</strong>
	<button
		class="quiet"
		onclick={() => onchange(addMonths(end, span), span)}
		disabled={end >= thisMonth()}
		aria-label="Later">→</button
	>
	<select
		value={String(span)}
		onchange={(e) => onchange(end, parseInt(e.currentTarget.value, 10))}
		aria-label="Months shown"
	>
		{#each spans as s (s)}<option value={String(s)}>{s} months</option>{/each}
	</select>
</span>

<style>
	.window {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
	.window button {
		padding: var(--space-1) var(--space-3);
	}
</style>
