<script lang="ts">
	import { api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import { today } from '#lib/dates.ts';
	import { referenceState } from '#lib/setup.svelte.ts';
	import { SETTLEMENT_CODE } from '#lib/usd.ts';

	type Price = Schemas['Price'];
	type Edit = { id: string; etag: string; rate: string };

	const { state: s } = referenceState();
	let filter = $state('');
	let prices: Price[] = $state([]);
	let next: string | null = $state(null);
	let editing: Edit | null = $state(null);
	let fresh = $state({ base_id: '', quote_id: '', date: today(), rate: '' });
	let failure: unknown = $state(null);
	let busy = $state(false);
	let refreshed: Schemas['Refreshed'] | null = $state(null);

	const code = (id: string) => s.ref?.commodities.get(id)?.code ?? '?';
	const sorted = $derived(
		[...(s.ref?.commodities.values() ?? [])].sort((a, b) => a.code.localeCompare(b.code))
	);
	/** `provider:tiingo` → `Tiingo`; the ECB in capitals. */
	const sourceName = (source: string) => {
		const name = source.replace(/^provider:/, '');
		if (source === 'manual') return 'Manual';
		return name.length <= 3 ? name.toUpperCase() : name[0].toUpperCase() + name.slice(1);
	};

	$effect(() => {
		if (!fresh.quote_id && s.ref) {
			fresh.quote_id = sorted.find((c) => c.code === SETTLEMENT_CODE)?.id ?? '';
		}
	});

	async function page(cursor?: string) {
		const r = ok(
			await api.GET('/api/v1/prices', {
				params: { query: { commodity_id: filter || undefined, limit: 100, cursor } }
			})
		);
		prices = cursor ? [...prices, ...r.items] : r.items;
		next = r.next_cursor ?? null;
	}

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			await page();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		void filter;
		run(async () => {});
	});

	const refresh = () =>
		run(async () => {
			refreshed = ok(await api.POST('/api/v1/prices/refresh', { body: {} }));
		});

	const add = () =>
		run(async () => {
			ok(await api.POST('/api/v1/prices', { body: { ...fresh, rate: fresh.rate.trim() } }));
			fresh.rate = '';
		});

	async function read(id: string) {
		const r = await api.GET('/api/v1/prices/{id}', { params: { path: { id } } });
		return { item: ok(r), etag: r.response.headers.get('etag') ?? '' };
	}

	async function edit(p: Price) {
		failure = null;
		try {
			const { item, etag } = await read(p.id);
			editing = { id: p.id, etag, rate: item.rate };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!editing) return;
			const e = editing;
			ok(
				await api.PATCH('/api/v1/prices/{id}', {
					params: { path: { id: e.id }, header: { 'If-Match': e.etag } },
					body: { rate: e.rate.trim() }
				})
			);
			editing = null;
		});

	const remove = (p: Price) =>
		run(async () => {
			const { etag } = await read(p.id);
			const what = `1 ${code(p.base_id)} = ${p.rate} ${code(p.quote_id)} on ${p.date}`;
			const again = p.source === 'manual' ? '' : ' It may be fetched again when needed.';
			if (!confirm(`Delete the price ${what}?${again}`)) return;
			done(
				await api.DELETE('/api/v1/prices/{id}', {
					params: { path: { id: p.id }, header: { 'If-Match': etag } }
				})
			);
		});
</script>

<div class="heading">
	<h1>Prices</h1>
	<button class="quiet" onclick={refresh} disabled={busy}>
		{busy && !refreshed ? 'Fetching…' : "Fetch today's prices"}
	</button>
	{#if refreshed}
		<span class="muted small">
			{refreshed.stored} stored{refreshed.sources.length
				? ` from ${refreshed.sources.map((n) => sourceName(`provider:${n}`)).join(', ')}`
				: ', no sources configured'}
		</span>
		{#each refreshed.errors as e (e)}<span class="warning small">{e}</span>{/each}
	{/if}
</div>

{#if s.ref}
	<form
		class="add"
		onsubmit={(e) => {
			e.preventDefault();
			add();
		}}
	>
		<span>1</span>
		<select bind:value={fresh.base_id} aria-label="Commodity">
			<option value="" disabled>Commodity</option>
			{#each sorted as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
		</select>
		<span>=</span>
		<input bind:value={fresh.rate} inputmode="decimal" placeholder="Rate" aria-label="Rate" />
		<select bind:value={fresh.quote_id} aria-label="In">
			{#each sorted as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
		</select>
		<label class="small">on <input type="date" bind:value={fresh.date} required /></label>
		<button type="submit" disabled={busy || !fresh.base_id || !fresh.quote_id || !fresh.rate.trim()}
			>Add</button
		>
	</form>

	<label class="small filter"
		>Show <select bind:value={filter}>
			<option value="">All commodities</option>
			{#each sorted as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
		</select></label
	>

	{#if failure}<Failure error={failure} />{/if}

	<div class="scroll">
		<table class="list">
			<thead>
				<tr><th>Date</th><th>Price</th><th>Source</th><th></th></tr>
			</thead>
			<tbody>
				{#each prices as p (p.id)}
					<tr class:editing={editing?.id === p.id}>
						<td class="date">{p.date}</td>
						<td>
							1 {code(p.base_id)} =
							{#if editing?.id === p.id}
								<input
									bind:value={editing.rate}
									inputmode="decimal"
									aria-label="Rate"
									class="rate"
								/>
								{code(p.quote_id)}
							{:else}
								<Amount value={p.rate} code={code(p.quote_id)} />
							{/if}
							{#if p.provisional}<span class="muted small">so far today</span>{/if}
						</td>
						<td>{sourceName(p.source)}</td>
						<td class="actions">
							{#if editing?.id === p.id}
								<button onclick={save} disabled={busy || !editing.rate.trim()}>Save</button>
								<button class="quiet" onclick={() => (editing = null)}>Cancel</button>
							{:else}
								{#if p.source === 'manual'}
									<button class="link" onclick={() => edit(p)} disabled={busy}>Edit</button>
								{/if}
								<button class="link danger" onclick={() => remove(p)} disabled={busy}>Delete</button
								>
							{/if}
						</td>
					</tr>
				{:else}
					<tr><td colspan="4" class="muted">No prices yet.</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
	{#if next}
		<button class="quiet more" onclick={() => run(() => page(next ?? undefined))} disabled={busy}
			>Show more</button
		>
	{/if}
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.heading {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2) var(--space-4);
		margin-bottom: var(--space-4);
	}
	.heading h1 {
		margin: 0;
	}
	.small {
		font-size: var(--text-sm);
	}
	.warning {
		color: var(--color-warning);
	}
	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.add input:not([type='date']),
	.rate {
		width: 8rem;
	}
	.filter {
		display: block;
		margin-bottom: var(--space-3);
		color: var(--color-text-muted);
	}
	.scroll {
		overflow-x: auto;
	}
	.list {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.date {
		white-space: nowrap;
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
	.more {
		margin-top: var(--space-3);
	}
</style>
