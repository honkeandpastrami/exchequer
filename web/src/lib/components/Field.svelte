<script lang="ts">
	// One field of a form, on lines of its own (ux.md §10): its name, its
	// control using the rest of the width, a badge at the end, and a line
	// under it that wraps (where the value came from). Wide, the name sits in
	// a column; narrow, above the control, the badge at the end of its line.
	import type { Snippet } from 'svelte';

	let {
		label,
		id,
		children,
		badge,
		status
	}: {
		label: string;
		/** The control's ID, for the name to label it. */
		id?: string;
		children: Snippet;
		badge?: Snippet;
		status?: Snippet;
	} = $props();
</script>

<div class="box">
	<div class="line">
		{#if id}<label class="name" for={id}>{label}</label>{:else}<span class="name">{label}</span
			>{/if}
		<div class="control">{@render children()}</div>
		<span class="badge">{@render badge?.()}</span>
		{#if status}<div class="status">{@render status()}</div>{/if}
	</div>
</div>

<style>
	.box {
		container-type: inline-size;
	}
	.line {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		grid-template-areas:
			'name badge'
			'control control'
			'status status';
		align-items: center;
		gap: var(--space-1) var(--space-3);
		padding: var(--space-1) 0;
	}
	@container (min-width: 36rem) {
		.line {
			grid-template-columns: 7rem minmax(0, 1fr) 3.5rem;
			grid-template-areas:
				'name control badge'
				'. status status';
		}
	}
	.name {
		grid-area: name;
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.control {
		grid-area: control;
		min-width: 0;
	}
	.control > :global(input:not([type='checkbox'], [type='radio'])),
	.control > :global(select),
	.control > :global(.picker) {
		width: 100%;
	}
	.badge {
		grid-area: badge;
		justify-self: end;
	}
	.status {
		grid-area: status;
		font-size: var(--text-xs);
		color: var(--color-text-muted);
		overflow-wrap: anywhere;
	}
	.status > :global(* + *) {
		margin-left: var(--space-1);
	}
</style>
