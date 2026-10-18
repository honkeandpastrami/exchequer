<script lang="ts">
	import { api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import { today } from '#lib/dates.ts';
	import { referenceState } from '#lib/setup.svelte.ts';
	import { SETTLEMENT_CODE } from '#lib/usd.ts';

	type Price = Schemas['Price'];
	/** The open form: a new price, or a price's (only a manual one's rate changes). */
	type Form =
		| { id: null; base_id: string; quote_id: string; date: string; rate: string }
		| { id: string; etag: string; rate: string; price: Price };

	const { state: s } = referenceState();
	let filter = $state('');
	let prices: Price[] = $state([]);
	let next: string | null = $state(null);
	let form: Form | null = $state(null);
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

	function adding() {
		failure = null;
		form = {
			id: null,
			base_id: '',
			quote_id: sorted.find((c) => c.code === SETTLEMENT_CODE)?.id ?? '',
			date: today(),
			rate: ''
		};
	}

	async function page(cursor?: string) {
		const r = ok(
			await api.GET('/api/v1/prices', {
				params: { query: { commodity_id: filter || undefined, limit: 100, cursor } }
			})
		);
		prices = cursor ? [...prices, ...r.items] : r.items;
		next = r.next_cursor ?? null;
	}

	async function run(action: () => Promise<unknown>, close = true) {
		busy = true;
		failure = null;
		try {
			await action();
			if (close) form = null;
			await page();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	$effect(() => {
		void filter;
		run(async () => {}, false);
	});

	const refresh = () =>
		run(async () => {
			refreshed = ok(await api.POST('/api/v1/prices/refresh', { body: {} }));
		}, false);

	async function read(id: string) {
		const r = await api.GET('/api/v1/prices/{id}', { params: { path: { id } } });
		return { item: ok(r), etag: r.response.headers.get('etag') ?? '' };
	}

	async function edit(p: Price) {
		if (form?.id === p.id) {
			form = null;
			return;
		}
		failure = null;
		try {
			const { item, etag } = await read(p.id);
			form = { id: p.id, etag, rate: item.rate, price: item };
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			const f = form;
			if (!f) return;
			if (f.id === null) {
				const body = {
					base_id: f.base_id,
					quote_id: f.quote_id,
					date: f.date,
					rate: f.rate.trim()
				};
				ok(await api.POST('/api/v1/prices', { body }));
			} else if (f.price.source === 'manual')
				ok(
					await api.PATCH('/api/v1/prices/{id}', {
						params: { path: { id: f.id }, header: { 'If-Match': f.etag } },
						body: { rate: f.rate.trim() }
					})
				);
		});

	async function remove(p: Price) {
		if (form?.id !== p.id) return;
		const etag = form.etag;
		const what = `1 ${code(p.base_id)} = ${p.rate} ${code(p.quote_id)} on ${p.date}`;
		const again = p.source === 'manual' ? '' : ' It may be fetched again when needed.';
		if (!confirm(`Delete the price ${what}?${again}`)) return;
		await run(async () =>
			done(
				await api.DELETE('/api/v1/prices/{id}', {
					params: { path: { id: p.id }, header: { 'If-Match': etag } }
				})
			)
		);
	}
</script>

{#snippet fields(f: Form)}
	<RowForm
		label={f.id ? 'Edit price' : 'Add price'}
		{busy}
		{failure}
		onsave={save}
		oncancel={() => (form = null)}
	>
		{#if f.id === null}
			<div class="fields">
				<label class="field">
					<span>1</span>
					<select bind:value={f.base_id} required aria-label="Commodity">
						<option value="" disabled>Commodity</option>
						{#each sorted as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
					</select>
				</label>
				<label class="field">
					<span>=</span>
					<input
						bind:value={f.rate}
						class="rate"
						inputmode="decimal"
						placeholder="Rate"
						required
						aria-label="Rate"
					/>
				</label>
				<label class="field">
					<span>In</span>
					<select bind:value={f.quote_id} required>
						{#each sorted as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
					</select>
				</label>
				<label class="field">
					<span>On</span>
					<input type="date" bind:value={f.date} required />
				</label>
			</div>
		{:else}
			{@const manual = f.price.source === 'manual'}
			<div class="fields">
				<label class="field">
					<span>1 {code(f.price.base_id)} =</span>
					<input
						bind:value={f.rate}
						class="rate"
						inputmode="decimal"
						required
						disabled={!manual}
						title={manual ? undefined : 'Fetched: delete it to fetch it again'}
					/>
				</label>
				<span class="in">{code(f.price.quote_id)} on {f.price.date}</span>
			</div>
		{/if}
		{#snippet actions()}
			{#if f.id}
				{@const p = f.price}
				<button type="button" class="quiet danger" onclick={() => remove(p)} disabled={busy}
					>Delete</button
				>
			{/if}
		{/snippet}
	</RowForm>
{/snippet}

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
	<button type="button" class="add" disabled={!s.ref || form?.id === null} onclick={adding}
		>Add price</button
	>
</div>

{#if s.ref}
	<label class="small filter"
		>Show <select bind:value={filter}>
			<option value="">All commodities</option>
			{#each sorted as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
		</select></label
	>

	<div class="scroll">
		<table class="list">
			<thead>
				<tr><th>Date</th><th>Price</th><th>Source</th><th></th></tr>
			</thead>
			<tbody>
				{#if form && form.id === null}
					<tr class="open"><td colspan="4">{@render fields(form)}</td></tr>
				{/if}
				{#each prices as p (p.id)}
					<tr>
						<td class="date">{p.date}</td>
						<td>
							1 {code(p.base_id)} = <Amount value={p.rate} code={code(p.quote_id)} />
							{#if p.provisional}<span class="muted small">so far today</span>{/if}
						</td>
						<td>{sourceName(p.source)}</td>
						<td class="actions">
							<button class="link" aria-expanded={form?.id === p.id} onclick={() => edit(p)}
								>Edit</button
							>
						</td>
					</tr>
					{#if form && form.id === p.id}
						<tr class="open"><td colspan="4">{@render fields(form)}</td></tr>
					{/if}
				{:else}
					{#if !form}<tr><td colspan="4" class="muted">No prices yet.</td></tr>{/if}
				{/each}
			</tbody>
		</table>
	</div>
	{#if failure && !form}<Failure error={failure} />{/if}
	{#if next}
		<button
			class="quiet more"
			onclick={() => run(() => page(next ?? undefined), false)}
			disabled={busy}>Show more</button
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
	.heading .add {
		margin-left: auto;
	}
	.rate {
		width: 8rem;
	}
	.in {
		padding-bottom: var(--space-2);
	}
	.open td {
		border-top: none;
		padding-top: 0;
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
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.more {
		margin-top: var(--space-3);
	}
</style>
