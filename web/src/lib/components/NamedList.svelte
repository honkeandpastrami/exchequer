<script lang="ts">
	// Categories, counterparties or tags: add, edit, delete (ux.md §10). Add
	// opens a form at the top of the list, Edit one under its row. Each edit
	// reads the item first, so it carries the ETag of what's stored now.
	import { resolve } from '$app/paths';
	import type { Named } from '../api/resources';
	import Failure from './Failure.svelte';
	import RowForm from './RowForm.svelte';

	type Item = { id: string; name: string; system?: string | null };
	type Form = { id: string | null; etag: string; name: string };

	let {
		items,
		resource,
		title,
		noun,
		filter,
		onchange
	}: {
		items: Item[];
		resource: Named;
		/** The page's heading: "Categories". */
		title: string;
		/** Singular, lowercase: "category". */
		noun: string;
		/** The transactions filter that shows where one is used. */
		filter: 'category_id' | 'counterparty_id' | 'tag_id';
		onchange: () => Promise<void>;
	} = $props();

	const SHOWN = 200;

	let search = $state('');
	/** The open form: a new one (`id` null) or an item's. */
	let form: Form | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);

	let matching = $derived(
		items
			.filter((i) => i.name.toLowerCase().includes(search.trim().toLowerCase()))
			.sort((a, b) => a.name.localeCompare(b.name))
	);
	const editing = (i: Item) => form?.id === i.id;

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			form = null;
			await onchange();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function edit(i: Item) {
		if (editing(i)) {
			form = null;
			return;
		}
		failure = null;
		try {
			const { item, etag } = await resource.read(i.id);
			form = { id: i.id, etag, name: item.name };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!form) return;
			const name = form.name.trim();
			if (form.id) await resource.update(form.id, form.etag, { name });
			else await resource.create({ name });
		});

	async function remove(i: Item) {
		if (!form || !confirm(`Delete the ${noun} “${i.name}”?`)) return;
		const etag = form.etag;
		await run(() => resource.remove(i.id, etag));
	}
</script>

{#snippet fields(f: Form, i?: Item)}
	<RowForm
		label={i ? `Edit ${i.name}` : `Add ${noun}`}
		{busy}
		{failure}
		onsave={save}
		oncancel={() => (form = null)}
	>
		<div class="fields">
			<label class="field grow">
				<span>Name</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input bind:value={f.name} maxlength="100" required autofocus />
			</label>
		</div>
		{#snippet actions()}
			{#if i && !i.system}
				<button type="button" class="quiet danger" onclick={() => remove(i)} disabled={busy}
					>Delete</button
				>
			{/if}
		{/snippet}
	</RowForm>
{/snippet}

<div class="heading">
	<h1>{title}</h1>
	<button
		type="button"
		disabled={form?.id === null}
		onclick={() => {
			failure = null;
			form = { id: null, etag: '', name: '' };
		}}>Add {noun}</button
	>
</div>
<div class="toolbar">
	{#if items.length > 20}
		<input type="search" bind:value={search} placeholder="Search" aria-label="Search" />
	{/if}
	<span class="muted count">{items.length} in all</span>
</div>

<table class="list">
	<tbody>
		{#if form && form.id === null}
			<tr class="open"><td colspan="2">{@render fields(form)}</td></tr>
		{/if}
		{#each matching.slice(0, SHOWN) as i (i.id)}
			<tr>
				<td>
					{i.name}
					{#if i.system}<span class="badge">built-in</span>{/if}
				</td>
				<td class="actions">
					<a href={resolve(`/transactions?${filter}=${i.id}`)}>Transactions</a>
					<button class="link" aria-expanded={editing(i)} onclick={() => edit(i)}>Edit</button>
				</td>
			</tr>
			{#if form && editing(i)}
				<tr class="open"><td colspan="2">{@render fields(form, i)}</td></tr>
			{/if}
		{:else}
			<tr><td class="muted">None{search ? ' match' : ' yet'}.</td></tr>
		{/each}
	</tbody>
</table>
{#if failure && !form}<Failure error={failure} />{/if}
{#if matching.length > SHOWN}
	<p class="muted">Showing {SHOWN} of {matching.length}; search to narrow them down.</p>
{/if}

<style>
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-4);
	}
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.count {
		margin-left: auto;
		font-size: var(--text-sm);
	}
	.list {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.open td {
		border-top: none;
		padding-top: 0;
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.actions > * + * {
		margin-left: var(--space-3);
	}
</style>
