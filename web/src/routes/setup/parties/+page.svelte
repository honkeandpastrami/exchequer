<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { parties } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import { partyKinds, rank } from '#lib/labels.ts';
	import { referenceState } from '#lib/setup.svelte.ts';

	type Kind = Schemas['PartyKind'];
	/** The open form: a new party (`id` null) or a party's. */
	type Form = { id: string | null; etag: string; name: string; kind: Kind };

	const { state: s, reload } = referenceState();
	let form: Form | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);

	const kindList = Object.entries(partyKinds) as [Kind, string][];

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			form = null;
			await reload();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function edit(p: Schemas['Party']) {
		if (form?.id === p.id) {
			form = null;
			return;
		}
		failure = null;
		try {
			const { item, etag } = await parties.read(p.id);
			form = { id: p.id, etag, name: item.name, kind: item.kind };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!form) return;
			const name = form.name.trim();
			if (form.id) await parties.update(form.id, form.etag, { name });
			else await parties.create({ name, kind: form.kind });
		});

	async function remove(p: Schemas['Party']) {
		if (!form || !confirm(`Delete “${p.name}”? Only a party nothing refers to can go.`)) return;
		const etag = form.etag;
		await run(() => parties.remove(p.id, etag));
	}
</script>

{#snippet fields(f: Form, p?: Schemas['Party'])}
	<RowForm
		label={p ? `Edit ${p.name}` : 'Add party'}
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
			<label class="field">
				<span>Kind</span>
				<!-- A party's kind is set when it's added. -->
				<select bind:value={f.kind} disabled={!!p}>
					{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
				</select>
			</label>
		</div>
		{#snippet actions()}
			{#if p}
				<button type="button" class="quiet danger" onclick={() => remove(p)} disabled={busy}
					>Delete</button
				>
			{/if}
		{/snippet}
	</RowForm>
{/snippet}

<div class="heading">
	<h1>Parties</h1>
	<button
		type="button"
		disabled={!s.ref || form?.id === null}
		onclick={() => {
			failure = null;
			form = { id: null, etag: '', name: '', kind: 'person' };
		}}>Add party</button
	>
</div>
<p class="muted intro">
	The household, the people in it and their businesses: who owns accounts and who money is for.
</p>

{#if s.ref}
	{@const r = s.ref}
	<table class="list">
		<thead>
			<tr><th>Name</th><th>Kind</th><th></th></tr>
		</thead>
		<tbody>
			{#if form && form.id === null}
				<tr class="open"><td colspan="3">{@render fields(form)}</td></tr>
			{/if}
			{#each [...r.parties.values()].sort((a, b) => rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)) as p (p.id)}
				<tr>
					<td>{p.name}</td>
					<td>{partyKinds[p.kind]}</td>
					<td class="actions">
						<button class="link" aria-expanded={form?.id === p.id} onclick={() => edit(p)}
							>Edit</button
						>
					</td>
				</tr>
				{#if form && form.id === p.id}
					<tr class="open"><td colspan="3">{@render fields(form, p)}</td></tr>
				{/if}
			{/each}
		</tbody>
	</table>
	{#if failure && !form}<Failure error={failure} />{/if}
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-4);
	}
	.intro {
		margin-bottom: var(--space-4);
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
</style>
