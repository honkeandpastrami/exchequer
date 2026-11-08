<script lang="ts">
	// A day in a form (docs/ux.md §11): typed (`2026-08-14` or `Aug 14,
	// 2026`) or picked from the calendar, and shown as `Aug 14, 2026` once
	// it's left. An optional one can be cleared. What doesn't read as a day
	// stays as typed, with the error under it (ux.md §8).
	import { dayName, parseDay } from '../../dates';
	import Calendar from './Calendar.svelte';

	let {
		value = $bindable(''),
		required = false,
		max,
		id,
		label,
		onchange
	}: {
		/** `YYYY-MM-DD`, or empty. */
		value?: string;
		required?: boolean;
		max?: string;
		/** The input's ID, for a label outside to name it. */
		id?: string;
		/** What it is, when no label outside names it. */
		label?: string;
		/** Called when the value changes, by typing, picking or clearing. */
		onchange?: (value: string) => void;
	} = $props();

	let text = $state(value ? dayName(value) : '');
	/** What's wrong with what was typed, if anything. */
	let wrong: string | null = $state(null);
	let opener: HTMLButtonElement | undefined = $state();
	let open = $state(false);

	// A value set from outside replaces what's shown; only a change counts,
	// so leaving the field with an error keeps what was typed.
	let shownFor = value;
	$effect(() => {
		if (value === shownFor) return;
		shownFor = value;
		text = value ? dayName(value) : '';
		wrong = null;
	});

	function set(v: string) {
		wrong = null;
		text = v ? dayName(v) : '';
		if (v === value) return;
		shownFor = v;
		value = v;
		onchange?.(v);
	}

	function leave() {
		if (text.trim() === '') {
			if (required) wrong = 'Pick a date.';
			else set('');
			return;
		}
		const day = parseDay(text);
		if (day && !(max !== undefined && day > max)) set(day);
		else if (day) wrong = `Pick ${dayName(max!)} or earlier.`;
		else wrong = 'Not a date: type it as Aug 14, 2026.';
	}
</script>

<span class="date-field">
	<span class="box" class:wrong={wrong !== null}>
		<input
			{id}
			type="text"
			autocomplete="off"
			placeholder="Mmm d, yyyy"
			aria-label={id ? undefined : label}
			aria-invalid={wrong !== null}
			{required}
			bind:value={text}
			onblur={leave}
			onkeydown={(e) => {
				if (e.key === 'Enter') {
					e.preventDefault();
					leave();
				}
			}}
		/>
		{#if !required}
			<button
				type="button"
				class="quiet"
				aria-label="Clear the date"
				disabled={!value && !text}
				onclick={() => set('')}>✕</button
			>
		{/if}
		<button
			bind:this={opener}
			type="button"
			class="quiet"
			aria-haspopup="dialog"
			aria-expanded={open}
			aria-label="Pick a date"
			onclick={() => (open = !open)}>▾</button
		>
	</span>
	{#if wrong}<span class="error" role="alert">{wrong}</span>{/if}
</span>
{#if open && opener}
	<Calendar
		{value}
		granularity="day"
		{max}
		anchor={opener}
		onpick={set}
		onclose={() => (open = false)}
	/>
{/if}

<style>
	.date-field {
		display: inline-flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	/* The input and its buttons are one control, never apart. */
	.box {
		display: inline-flex;
		align-items: stretch;
		gap: var(--space-1);
		white-space: nowrap;
	}
	input {
		width: 9.5em;
		font-variant-numeric: var(--numeric);
	}
	.wrong input {
		border-color: var(--color-negative);
	}
	.error {
		color: var(--color-negative);
		font-size: var(--text-sm);
	}
</style>
