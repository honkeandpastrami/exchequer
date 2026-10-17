<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { parties } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { partyKinds, rank } from '#lib/labels.ts';
	import { referenceState } from '#lib/setup.svelte.ts';

	type Kind = Schemas['PartyKind'];
	type Edit = { id: string; etag: string; name: string };

	const { state: s, reload } = referenceState();
	let editing: Edit | null = $state(null);
	let fresh = $state({ name: '', kind: 'person' as Kind });
	let failure: unknown = $state(null);
	let busy = $state(false);

	const kindList = Object.entries(partyKinds) as [Kind, string][];

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			await reload();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function edit(p: Schemas['Party']) {
		failure = null;
		try {
			const { item, etag } = await parties.read(p.id);
			editing = { id: p.id, etag, name: item.name };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!editing) return;
			const e = editing;
			await parties.update(e.id, e.etag, { name: e.name.trim() });
			editing = null;
		});

	const remove = (p: Schemas['Party']) =>
		run(async () => {
			const { etag } = await parties.read(p.id);
			if (!confirm(`Delete “${p.name}”? Only a party nothing refers to can go.`)) return;
			await parties.remove(p.id, etag);
		});

	const add = () =>
		run(async () => {
			await parties.create({ name: fresh.name.trim(), kind: fresh.kind });
			fresh.name = '';
		});
</script>

<h1>Parties</h1>
<p class="muted intro">
	The household, the people in it and their businesses: who owns accounts and who money is for.
</p>

{#if s.ref}
	{@const r = s.ref}
	<form
		class="add"
		onsubmit={(e) => {
			e.preventDefault();
			add();
		}}
	>
		<input bind:value={fresh.name} placeholder="New party" maxlength="100" aria-label="Name" />
		<select bind:value={fresh.kind} aria-label="Kind">
			{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
		</select>
		<button type="submit" disabled={busy || !fresh.name.trim()}>Add</button>
	</form>

	{#if failure}<Failure error={failure} />{/if}

	<table class="list">
		<thead>
			<tr><th>Name</th><th>Kind</th><th></th></tr>
		</thead>
		<tbody>
			{#each [...r.parties.values()].sort((a, b) => rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)) as p (p.id)}
				{#if editing?.id === p.id}
					<tr class="editing">
						<td><input bind:value={editing.name} maxlength="100" aria-label="Name" /></td>
						<td>{partyKinds[p.kind]}</td>
						<td class="actions">
							<button onclick={save} disabled={busy || !editing.name.trim()}>Save</button>
							<button class="quiet" onclick={() => (editing = null)}>Cancel</button>
						</td>
					</tr>
				{:else}
					<tr>
						<td>{p.name}</td>
						<td>{partyKinds[p.kind]}</td>
						<td class="actions">
							<button class="link" onclick={() => edit(p)} disabled={busy}>Edit</button>
							<button class="link danger" onclick={() => remove(p)} disabled={busy}>Delete</button>
						</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.intro,
	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.list {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.editing td {
		background: var(--color-surface-alt);
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.actions > * + * {
		margin-left: var(--space-3);
	}
</style>
