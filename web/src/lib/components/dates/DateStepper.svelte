<script lang="ts">
	// A page's current day, month or year (docs/ux.md §11): earlier, the
	// period (which opens the calendar), later, and Today. One look on every
	// page; only the label and the step change. The label keeps the width of
	// its longest value, and later and Today are disabled, never hidden, so
	// stepping never moves anything (ux.md §2).
	import { current, periodName, step, type Granularity } from '../../dates';
	import Calendar from './Calendar.svelte';

	let {
		value,
		granularity,
		max,
		onchange,
		by = 1,
		label,
		widest,
		name,
		today = true
	}: {
		value: string;
		granularity: Granularity;
		/** The latest value allowed, at `granularity`. */
		max?: string;
		onchange: (value: string) => void;
		/** How many units a step moves. */
		by?: number;
		/** The label, when it isn't the value's own name (a range). */
		label?: string;
		/** A label at least as wide as any this stepper shows. */
		widest?: string;
		/** What the period is, for screen readers. */
		name: string;
		/** Whether Today sits in the stepper; a narrow range puts it on its own row. */
		today?: boolean;
	} = $props();

	const widths: Record<Granularity, string> = {
		day: 'Mmm 00, 0000',
		month: 'Mmm 0000',
		year: '0000'
	};

	let opener: HTMLButtonElement | undefined = $state();
	let open = $state(false);

	const clamp = (v: string) => (max !== undefined && v > max ? max : v);
	let latest = $derived(max !== undefined && value >= max);
	let now = $derived(clamp(current(granularity)));
</script>

<span class="stepper" role="group" aria-label={name}>
	<button
		type="button"
		class="quiet"
		aria-label="Earlier"
		onclick={() => onchange(step(value, granularity, -by))}>‹</button
	>
	<button
		bind:this={opener}
		type="button"
		class="quiet period"
		aria-haspopup="dialog"
		aria-expanded={open}
		aria-label="{name}: {label ?? periodName(value, granularity)}, pick another"
		onclick={() => (open = !open)}
	>
		<span class="sizer">
			<span>{label ?? periodName(value, granularity)}</span>
			<span class="widest" aria-hidden="true">{widest ?? widths[granularity]}</span>
		</span>
		<span aria-hidden="true">▾</span>
	</button>
	<button
		type="button"
		class="quiet"
		aria-label="Later"
		disabled={latest}
		onclick={() => onchange(clamp(step(value, granularity, by)))}>›</button
	>
	{#if today}
		<button type="button" class="quiet" disabled={value === now} onclick={() => onchange(now)}
			>Today</button
		>
	{/if}
</span>
{#if open && opener}
	<Calendar
		{value}
		{granularity}
		{max}
		anchor={opener}
		onpick={onchange}
		onclose={() => (open = false)}
	/>
{/if}

<style>
	/* One unit: it never wraps piece by piece (ux.md §3). The size of the
	   content it sits over, not of the page's headings. */
	.stepper {
		font-size: var(--text-sm);
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		white-space: nowrap;
	}
	.period {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-variant-numeric: var(--numeric);
	}
	/* The label and its widest sample share one cell, so the button is as
	   wide as the widest label it will show. */
	.sizer {
		display: inline-grid;
	}
	.sizer > * {
		grid-area: 1 / 1;
	}
	.widest {
		visibility: hidden;
	}
</style>
