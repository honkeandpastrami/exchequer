<script lang="ts">
	// Categories, counterparties or tags: add, rename, delete. Each edit reads
	// the item first, so it carries the ETag of what's stored now.
	import { resolve } from '$app/paths';
	import type { Named } from '../api/resources';
	import Failure from './Failure.svelte';

	type Item = { id: string; name: string; system?: string | null };

	let {
		items,
		resource,
		noun,
		filter,
		onchange
	}: {
		items: Item[];
		resource: Named;
		/** Singular, lowercase: "category". */
		noun: string;
		/** The transactions filter that shows where one is used. */
		filter: 'category_id' | 'counterparty_id' | 'tag_id';
		onchange: () => Promise<void>;
	} = $props();

	const SHOWN = 200;

	let search = $state('');
	let fresh = $state('');
	let editing: { id: string; name: string; etag: string } | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);

	let matching = $derived(
		items
			.filter((i) => i.name.toLowerCase().includes(search.trim().toLowerCase()))
			.sort((a, b) => a.name.localeCompare(b.name))
	);

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			await onchange();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	const add = () =>
		run(async () => {
			await resource.create({ name: fresh.trim() });
			fresh = '';
		});

	async function startRename(i: Item) {
		failure = null;
		try {
			const { item, etag } = await resource.read(i.id);
			editing = { id: i.id, name: item.name, etag };
		} catch (e) {
			failure = e;
		}
	}

	const rename = () =>
		run(async () => {
			if (!editing) return;
			await resource.update(editing.id, editing.etag, { name: editing.name.trim() });
			editing = null;
		});

	const remove = (i: Item) =>
		run(async () => {
			const { etag } = await resource.read(i.id);
			if (!confirm(`Delete the ${noun} “${i.name}”?`)) return;
			await resource.remove(i.id, etag);
		});
</script>

<form
	class="add"
	onsubmit={(e) => {
		e.preventDefault();
		if (fresh.trim()) add();
	}}
>
	<input bind:value={fresh} placeholder="New {noun}" maxlength="100" aria-label="New {noun}" />
	<button type="submit" disabled={busy || !fresh.trim()}>Add</button>
	{#if items.length > 20}
		<input class="search" type="search" bind:value={search} placeholder="Search" />
	{/if}
	<span class="muted count">{items.length} in all</span>
</form>

{#if failure}<Failure error={failure} />{/if}

<table class="list">
	<tbody>
		{#each matching.slice(0, SHOWN) as i (i.id)}
			<tr>
				{#if editing?.id === i.id}
					<td colspan="2">
						<form
							class="rename"
							onsubmit={(e) => {
								e.preventDefault();
								rename();
							}}
						>
							<!-- svelte-ignore a11y_autofocus -->
							<input bind:value={editing.name} maxlength="100" autofocus aria-label="Name" />
							<button type="submit" disabled={busy || !editing.name.trim()}>Save</button>
							<button type="button" class="quiet" onclick={() => (editing = null)}>Cancel</button>
						</form>
					</td>
				{:else}
					<td>
						{i.name}
						{#if i.system}<span class="badge">built-in</span>{/if}
					</td>
					<td class="actions">
						<a href={resolve(`/transactions?${filter}=${i.id}`)}>Transactions</a>
						<button class="link" onclick={() => startRename(i)} disabled={busy}>Rename</button>
						{#if !i.system}
							<button class="link danger" onclick={() => remove(i)} disabled={busy}>Delete</button>
						{/if}
					</td>
				{/if}
			</tr>
		{:else}
			<tr><td class="muted">None{search ? ' match' : ' yet'}.</td></tr>
		{/each}
	</tbody>
</table>
{#if matching.length > SHOWN}
	<p class="muted">Showing {SHOWN} of {matching.length}; search to narrow them down.</p>
{/if}

<style>
	.add,
	.rename {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
	}
	.add {
		margin-bottom: var(--space-4);
	}
	.search {
		margin-left: auto;
	}
	.count {
		font-size: var(--text-sm);
	}
	.list {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.actions > * + * {
		margin-left: var(--space-3);
	}
</style>
