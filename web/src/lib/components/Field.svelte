<script lang="ts">
	// One field of a form, on a line of its own (docs/ux.md §7): its name, its
	// control and its badge, and a line under it when there's something to
	// say. The form's section picks the look (`Section`): wide, the name sits
	// in a column and the control has a width of its own; narrow, the name
	// is above the control, the badge at the end of its line.
	import type { Snippet } from 'svelte';
	import { look } from './Section.svelte';

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

	const section = look();
</script>

<div class="line" class:narrow={section.look === 'narrow'}>
	{#if id}<label class="name" for={id}>{label}</label>{:else}<span class="name">{label}</span>{/if}
	<div class="control">{@render children()}</div>
	<span class="badge">{@render badge?.()}</span>
	{#if status}<div class="status">{@render status()}</div>{/if}
</div>

<style>
	.line {
		display: grid;
		grid-template-columns: 7rem minmax(14rem, var(--field-max)) max-content;
		grid-template-areas:
			'name control badge'
			'. status status';
		justify-content: start;
		align-items: center;
		gap: var(--space-1) var(--space-3);
	}
	.line.narrow {
		grid-template-columns: minmax(0, 1fr) auto;
		grid-template-areas:
			'name badge'
			'control control'
			'status status';
		justify-content: stretch;
	}
	.name {
		grid-area: name;
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
		justify-self: start;
	}
	.narrow .badge {
		justify-self: end;
	}
	.status {
		grid-area: status;
		color: var(--color-text-muted);
		overflow-wrap: anywhere;
	}
	.status > :global(* + *) {
		margin-left: var(--space-1);
	}
</style>
