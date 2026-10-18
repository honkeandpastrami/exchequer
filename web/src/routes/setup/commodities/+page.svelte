<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { commodities } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import { referenceState } from '#lib/setup.svelte.ts';

	type Kind = Schemas['CommodityKind'];
	/** The open form: a new commodity (`id` null) or a commodity's. */
	type Form = { id: string | null; etag: string; code: string; kind: Kind; precision: number };

	const kinds: Record<Kind, string> = {
		fiat: 'Currency',
		security: 'Security',
		fund: 'Fund',
		crypto: 'Crypto',
		other: 'Other'
	};
	const kindList = Object.entries(kinds) as [Kind, string][];

	const { state: s, reload } = referenceState();
	let form: Form | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);

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

	async function edit(c: Schemas['Commodity']) {
		if (form?.id === c.id) {
			form = null;
			return;
		}
		failure = null;
		try {
			const { item, etag } = await commodities.read(c.id);
			form = { id: c.id, etag, code: item.code, kind: item.kind, precision: item.precision };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!form) return;
			const body = { code: form.code.trim(), kind: form.kind, precision: form.precision };
			if (form.id) await commodities.update(form.id, form.etag, body);
			else await commodities.create(body);
		});

	async function remove(c: Schemas['Commodity']) {
		if (!form || !confirm(`Delete ${c.code}? Only a commodity nothing refers to can go.`)) return;
		const etag = form.etag;
		await run(() => commodities.remove(c.id, etag));
	}
</script>

{#snippet fields(f: Form, c?: Schemas['Commodity'])}
	<RowForm
		label={c ? `Edit ${c.code}` : 'Add commodity'}
		{busy}
		{failure}
		onsave={save}
		oncancel={() => (form = null)}
	>
		<div class="fields">
			<label class="field">
				<span>Code</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input bind:value={f.code} placeholder="e.g. EUR" required autofocus />
			</label>
			<label class="field">
				<span>Kind</span>
				<select bind:value={f.kind}>
					{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
				</select>
			</label>
			<label class="field">
				<span>Decimals</span>
				<!-- Decimals only grow: amounts already stored keep theirs. -->
				<input
					type="number"
					min={c?.precision ?? 0}
					max="18"
					bind:value={f.precision}
					class="precision"
				/>
			</label>
		</div>
		{#snippet actions()}
			{#if c}
				<button type="button" class="quiet danger" onclick={() => remove(c)} disabled={busy}
					>Delete</button
				>
			{/if}
		{/snippet}
	</RowForm>
{/snippet}

<div class="heading">
	<h1>Commodities</h1>
	<button
		type="button"
		disabled={!s.ref || form?.id === null}
		onclick={() => {
			failure = null;
			form = { id: null, etag: '', code: '', kind: 'fiat', precision: 2 };
		}}>Add commodity</button
	>
</div>

{#if s.ref}
	{@const r = s.ref}
	<table class="list">
		<thead>
			<tr><th>Code</th><th>Kind</th><th class="num">Decimals</th><th></th></tr>
		</thead>
		<tbody>
			{#if form && form.id === null}
				<tr class="open"><td colspan="4">{@render fields(form)}</td></tr>
			{/if}
			{#each [...r.commodities.values()].sort((a, b) => a.code.localeCompare(b.code)) as c (c.id)}
				<tr>
					<td>{c.code}</td>
					<td>{kinds[c.kind]}</td>
					<td class="num">{c.precision}</td>
					<td class="actions">
						<button class="link" aria-expanded={form?.id === c.id} onclick={() => edit(c)}
							>Edit</button
						>
					</td>
				</tr>
				{#if form && form.id === c.id}
					<tr class="open"><td colspan="4">{@render fields(form, c)}</td></tr>
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
	.precision {
		width: 5rem;
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
