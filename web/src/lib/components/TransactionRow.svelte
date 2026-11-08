<script lang="ts">
	// A transaction in a list, the same on every page that shows them
	// (docs/ux.md §5): its first line (date, description, amount), a status
	// sentence (`RowStatus`), its values one per line (`Values`), its actions,
	// and its form, open under it. The list is one section (`Section`): wide,
	// the actions are a column on the right; narrow, the row is one block, its
	// actions at the bottom. Actions are boxes (`.box`) using the column's
	// width; `.tools` puts the ones after the choices side by side when narrow.
	import { dayName } from '../dates';
	import type { Snippet } from 'svelte';
	import type { Tone } from './Mark.svelte';
	import { look } from './Section.svelte';

	let {
		id,
		label,
		date,
		tone,
		flash = false,
		muted = false,
		description,
		amount,
		status,
		values,
		actions,
		form
	}: {
		id?: string;
		/** What the row is, for screen readers. */
		label: string;
		date: string;
		/** Its colour, as a strip down its left edge. */
		tone?: Tone;
		/** Just brought into view: a highlight that fades. */
		flash?: boolean;
		/** Out of the way (not added): its text muted, its actions not. */
		muted?: boolean;
		description: Snippet;
		amount: Snippet;
		status?: Snippet;
		values?: Snippet;
		actions?: Snippet;
		/** Its form, when open. */
		form?: Snippet;
	} = $props();

	const section = look();
</script>

<article
	{id}
	class="row {tone ?? ''}"
	class:narrow={section.look === 'narrow'}
	class:flash
	class:muted
	aria-label={label}
>
	<div class="date">{dayName(date)}</div>
	<div class="desc">{@render description()}</div>
	<div class="amount">{@render amount()}</div>
	{#if status}<div class="status">{@render status()}</div>{/if}
	{#if values}<div class="values">{@render values()}</div>{/if}
	{#if actions}<div class="actions">{@render actions()}</div>{/if}
	{#if form}<div class="form">{@render form()}</div>{/if}
</article>

<style>
	.row {
		--strip: transparent;
		display: grid;
		grid-template-columns: 6.5rem minmax(12rem, 1fr) max-content var(--row-actions);
		/* Taller actions grow the values' row, never the first line. */
		grid-template-rows: auto auto 1fr auto;
		grid-template-areas:
			'date desc amount actions'
			'status status status actions'
			'values values values actions'
			'form form form form';
		gap: var(--space-1) var(--space-3);
		padding: var(--space-2) var(--space-3);
		border-left: 4px solid var(--strip);
		border-top: var(--border-width) solid var(--color-border);
	}
	.row:first-child {
		border-top: none;
	}
	.row.narrow {
		grid-template-columns: minmax(0, 1fr) auto;
		grid-template-rows: none;
		grid-template-areas:
			'date amount'
			'desc desc'
			'status status'
			'values values'
			'actions actions'
			'form form';
	}
	.error {
		--strip: var(--color-negative);
	}
	.warning {
		--strip: var(--color-warning);
	}
	.ok {
		--strip: var(--color-positive);
	}
	.off {
		--strip: var(--color-smoke);
	}
	.muted > div:not(.actions, .form) {
		color: var(--color-text-muted);
	}
	.date {
		grid-area: date;
		white-space: nowrap;
		font-variant-numeric: var(--numeric);
	}
	.desc {
		grid-area: desc;
		overflow-wrap: anywhere;
	}
	.amount {
		grid-area: amount;
		text-align: right;
		white-space: nowrap;
	}
	.status {
		grid-area: status;
		min-width: 0;
	}
	.values {
		grid-area: values;
		align-self: start;
		min-width: 0;
	}
	.actions {
		grid-area: actions;
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}
	.narrow .actions :global(.tools) {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: 1fr;
		gap: var(--space-1);
	}
	.actions :global(.tools) {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.form {
		grid-area: form;
		min-width: 0;
	}
	/* An action: a box filling the column's width, all of it the click area. */
	.actions :global(.box) {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		width: 100%;
		margin: 0;
		padding: var(--space-1) var(--space-2);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-sm);
		background: var(--color-surface);
		color: var(--color-text);
		text-align: left;
		cursor: pointer;
	}
	.actions :global(.box:hover) {
		background: var(--color-surface-alt);
	}
	.actions :global(.box:has(input:checked)),
	.actions :global(.box[aria-expanded='true']) {
		border-color: var(--color-accent);
		background: var(--color-surface-alt);
	}
	.flash {
		animation: flash 2s ease-out;
	}
	@keyframes flash {
		from {
			background: var(--color-warning-bg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.flash {
			animation: none;
			background: var(--color-warning-bg);
		}
	}
</style>
