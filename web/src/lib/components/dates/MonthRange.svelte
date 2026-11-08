<script lang="ts">
	// A run of whole months ending at `end` (docs/ux.md §11): the month
	// stepper, labelled with the range and stepping a whole window, plus how
	// many months it spans. The calendar picks the last month. It's a section of
	// its own, so it switches looks by itself wherever it sits (ux.md §3):
	// narrow, Today and the span move to a second row.
	import { addMonths, rangeName, thisMonth } from '../../dates';
	import Section from '../Section.svelte';
	import DateStepper from './DateStepper.svelte';

	let {
		end,
		span,
		onchange
	}: { end: string; span: number; onchange: (end: string, span: number) => void } = $props();

	const spans = [3, 6, 12, 24, 60];
	const now = thisMonth();
</script>

<Section label="Months shown">
	{#snippet children(l)}
		<span class="range" class:narrow={l === 'narrow'}>
			<DateStepper
				value={end}
				granularity="month"
				max={now}
				by={span}
				label={rangeName(addMonths(end, 1 - span), end)}
				widest="Mmm 0000 – Mmm 0000"
				name="Months shown"
				today={l === 'wide'}
				onchange={(e) => onchange(e, span)}
			/>
			<span class="second">
				{#if l === 'narrow'}
					<button
						type="button"
						class="quiet"
						disabled={end === now}
						onclick={() => onchange(now, span)}>Today</button
					>
				{/if}
				<select
					value={String(span)}
					onchange={(e) => onchange(end, parseInt(e.currentTarget.value, 10))}
					aria-label="Months shown"
				>
					{#each spans as s (s)}<option value={String(s)}>{s} months</option>{/each}
				</select>
			</span>
		</span>
	{/snippet}
</Section>

<style>
	/* Wide, one row; narrow, Today and the span under the stepper. Never
	   anything in between (ux.md §3). */
	.range {
		font-size: var(--text-sm);
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		white-space: nowrap;
	}
	.range.narrow {
		display: inline-grid;
		justify-items: start;
	}
	.second {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}
</style>
