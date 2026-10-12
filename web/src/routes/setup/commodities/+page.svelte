<script lang="ts">
	import type { Schemas } from '#lib/api/client.ts';
	import { commodities } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { referenceState } from '#lib/setup.svelte.ts';

	type Kind = Schemas['CommodityKind'];
	type Edit = { id: string; etag: string; code: string; kind: Kind; precision: number };

	const kinds: Record<Kind, string> = {
		fiat: 'Currency',
		security: 'Security',
		fund: 'Fund',
		crypto: 'Crypto',
		other: 'Other'
	};
	const kindList = Object.entries(kinds) as [Kind, string][];

	const { state: s, reload } = referenceState();
	let editing: Edit | null = $state(null);
	let fresh = $state({ code: '', kind: 'fiat' as Kind, precision: 2 });
	let failure: unknown = $state(null);
	let busy = $state(false);

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

	async function edit(c: Schemas['Commodity']) {
		failure = null;
		try {
			const { item, etag } = await commodities.read(c.id);
			editing = { id: c.id, etag, code: item.code, kind: item.kind, precision: item.precision };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!editing) return;
			const e = editing;
			await commodities.update(e.id, e.etag, {
				code: e.code.trim(),
				kind: e.kind,
				precision: e.precision
			});
			editing = null;
		});

	const remove = (c: Schemas['Commodity']) =>
		run(async () => {
			const { etag } = await commodities.read(c.id);
			if (!confirm(`Delete ${c.code}? Only a commodity nothing refers to can go.`)) return;
			await commodities.remove(c.id, etag);
		});

	const add = () =>
		run(async () => {
			await commodities.create({ ...fresh, code: fresh.code.trim() });
			fresh.code = '';
		});
</script>

<h1>Commodities</h1>

{#if s.ref}
	{@const r = s.ref}
	<form
		class="add"
		onsubmit={(e) => {
			e.preventDefault();
			add();
		}}
	>
		<input bind:value={fresh.code} placeholder="Code, e.g. EUR" aria-label="Code" />
		<select bind:value={fresh.kind} aria-label="Kind">
			{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
		</select>
		<label class="small"
			>Decimals <input
				type="number"
				min="0"
				max="18"
				bind:value={fresh.precision}
				class="precision"
			/></label
		>
		<button type="submit" disabled={busy || !fresh.code.trim()}>Add</button>
	</form>

	{#if failure}<Failure error={failure} />{/if}

	<table class="list">
		<thead>
			<tr><th>Code</th><th>Kind</th><th class="num">Decimals</th><th></th></tr>
		</thead>
		<tbody>
			{#each [...r.commodities.values()].sort((a, b) => a.code.localeCompare(b.code)) as c (c.id)}
				{#if editing?.id === c.id}
					<tr class="editing">
						<td><input bind:value={editing.code} aria-label="Code" /></td>
						<td>
							<select bind:value={editing.kind} aria-label="Kind">
								{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
							</select>
						</td>
						<td class="num">
							<input
								type="number"
								min={c.precision}
								max="18"
								bind:value={editing.precision}
								class="precision"
								aria-label="Decimals"
							/>
						</td>
						<td class="actions">
							<button onclick={save} disabled={busy || !editing.code.trim()}>Save</button>
							<button class="quiet" onclick={() => (editing = null)}>Cancel</button>
						</td>
					</tr>
				{:else}
					<tr>
						<td>{c.code}</td>
						<td>{kinds[c.kind]}</td>
						<td class="num">{c.precision}</td>
						<td class="actions">
							<button class="link" onclick={() => edit(c)} disabled={busy}>Edit</button>
							<button class="link danger" onclick={() => remove(c)} disabled={busy}>Delete</button>
						</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>
	<p class="muted note">
		Decimals are how many places an amount may have (2 for USD, 0 for JPY). They can only go up:
		lowering them could invalidate amounts already stored.
	</p>
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.small,
	.note {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
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
