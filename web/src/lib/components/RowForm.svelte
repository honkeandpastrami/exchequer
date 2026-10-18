<script lang="ts">
	// The form that opens in place, under a row or at the top of a list (ux.md
	// §10): its fields, what's wrong with them, Save and Cancel, and facts
	// about the item at the bottom.
	import type { Snippet } from 'svelte';
	import { ApiError } from '../api/client';
	import Failure from './Failure.svelte';

	let {
		label,
		busy,
		issues = [],
		failure = null,
		onsave,
		oncancel,
		onreload,
		children,
		actions,
		facts
	}: {
		/** What the form is about, for screen readers: "Edit Grocery store". */
		label: string;
		busy: boolean;
		/** Problems the form points out before asking the server. */
		issues?: string[];
		failure?: unknown;
		onsave: () => void;
		oncancel: () => void;
		/** Read the item again, after someone else changed it (412). */
		onreload?: () => void;
		children: Snippet;
		/** More actions, after Cancel: Copy as new, Delete. */
		actions?: Snippet;
		/** Facts about the item: its ID, where it came from. */
		facts?: Snippet;
	} = $props();
</script>

<form
	class="row-form"
	aria-label={label}
	onsubmit={(e) => {
		e.preventDefault();
		onsave();
	}}
>
	{@render children()}

	{#if issues.length}
		<ul class="issues" role="alert">
			{#each issues as m (m)}<li>{m}</li>{/each}
		</ul>
	{/if}
	{#if failure}
		<Failure error={failure} />
		{#if onreload && failure instanceof ApiError && failure.status === 412}
			<button type="button" class="quiet" onclick={onreload}>Reload, discarding my changes</button>
		{/if}
	{/if}

	<div class="actions">
		<button type="submit" disabled={busy}>{busy ? 'Saving…' : 'Save'}</button>
		<button type="button" class="quiet" onclick={oncancel} disabled={busy}>Cancel</button>
		{#if actions}<span class="more">{@render actions()}</span>{/if}
	</div>
	{#if facts}<p class="facts muted">{@render facts()}</p>{/if}
</form>

<style>
	.row-form {
		padding: var(--space-3) var(--space-4);
		background: var(--color-surface-alt);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
	}
	/* Fields in a form: each a label over its control, wrapping as needed. */
	.row-form :global(.fields) {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}
	.row-form :global(.field) {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.row-form :global(.field > span:first-child),
	.row-form :global(.check) {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.row-form :global(.check) {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding-bottom: var(--space-2);
	}
	.row-form :global(.grow) {
		flex: 1;
		min-width: 14rem;
	}
	.row-form :global(.danger) {
		color: var(--color-negative);
		border-color: var(--color-negative);
	}
	.issues {
		padding: var(--space-3) var(--space-5);
		border-radius: var(--radius-sm);
		background: var(--color-warning-bg);
		color: var(--color-warning);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-top: var(--space-3);
	}
	.more {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-left: auto;
	}
	.facts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: var(--space-2) 0 0;
		font-size: var(--text-xs);
		font-family: var(--font-mono);
	}
</style>
