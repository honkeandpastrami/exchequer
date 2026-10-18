<script lang="ts">
	// The Transactions list: every transaction matching the filters, or just
	// one (`/transactions/<id>`). A row opens its form in place, and Add opens
	// a blank one at the top.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import Quantities from '#lib/components/Quantities.svelte';
	import TransactionForm from '#lib/components/TransactionForm.svelte';
	import { hashQuery, withParam } from '#lib/query.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { reference, sorted, type Reference } from '#lib/reference.ts';
	import {
		categoryNames,
		effectOn,
		route,
		runningBalances,
		size
	} from '#lib/views/transactions.ts';
	import { batchName } from '#lib/views/batches.ts';
	import { localDate } from '#lib/dates.ts';

	const KEYS = [
		'q',
		'from',
		'to',
		'account_id',
		'counterparty_id',
		'category_id',
		'tag_id',
		'import_batch_id'
	] as const;
	type Filters = Partial<Record<(typeof KEYS)[number], string>>;
	const PAGE = 100;

	/** Show only this transaction. */
	let { only }: { only: string | null } = $props();

	let query = $derived(hashQuery(page.url));
	let filters: Filters = $derived(
		Object.fromEntries(KEYS.map((k) => [k, query.get(k)]).filter(([, v]) => v)) as Filters
	);

	function set(key: (typeof KEYS)[number], value: string) {
		const next = withParam(query, key, value);
		goto(resolve(next ? `/transactions?${next}` : '/transactions'), {
			replace: true,
			reset: false
		});
	}

	// The search box waits for a pause in typing.
	let text = $state(hashQuery(page.url).get('q') ?? '');
	let timer: ReturnType<typeof setTimeout> | undefined;
	function typed() {
		clearTimeout(timer);
		timer = setTimeout(() => set('q', text.trim()), 300);
	}

	let items: Schemas['Transaction'][] = $state([]);
	let next: string | null = $state(null);
	let loading = $state(false);
	let failure: unknown = $state(null);
	// Shown alone, a transaction comes with its form open (the route keys this component).
	let open: Record<string, boolean> = $state(untrack(() => (only ? { [only]: true } : {})));
	/** The form at the top: a new transaction, maybe like an existing one. */
	let adding: { copy?: string } | null = $state(null);
	let top: HTMLElement | undefined = $state();
	// One account's complete history (dates aside): it gets a running balance.
	let history = $derived(
		!only &&
			!!filters.account_id &&
			Object.keys(filters).every((k) => ['account_id', 'from', 'to'].includes(k))
	);
	let opening: Map<string, string> | null = $state(null);
	let generation = 0;

	async function fetchPage(f: Filters, cursor: string | null) {
		const mine = cursor ? generation : ++generation;
		loading = true;
		failure = null;
		try {
			if (only) {
				const tx = ok(
					await api.GET('/api/v1/transactions/{id}', { params: { path: { id: only } } })
				);
				if (mine !== generation) return;
				items = [tx];
				next = null;
				return;
			}
			const r = ok(
				await api.GET('/api/v1/transactions', {
					params: { query: { ...f, limit: PAGE, cursor: cursor ?? undefined } }
				})
			);
			if (mine !== generation) return; // the filters changed meanwhile
			if (!cursor && history && f.account_id && r.items.length) {
				const b = ok(
					await api.GET('/api/v1/reports/balances', {
						params: { query: { date: r.items[0].date } }
					})
				);
				if (mine !== generation) return;
				opening = new Map(
					b.balances
						.filter((x) => x.account_id === f.account_id)
						.map((x) => [x.commodity_id, x.balance])
				);
			}
			items = cursor ? [...items, ...r.items] : r.items;
			next = r.next_cursor ?? null;
		} catch (e) {
			if (mine === generation) failure = e;
		} finally {
			if (mine === generation) loading = false;
		}
	}

	// The batch being shown, when filtering by one.
	let batch: Schemas['ImportBatch'] | null = $state(null);
	$effect(() => {
		const id = filters.import_batch_id;
		batch = null;
		if (!id) return;
		api
			.GET('/api/v1/import-batches/{id}', { params: { path: { id } } })
			.then((r) => {
				if (filters.import_batch_id === id) batch = ok(r);
			})
			.catch((e) => (failure = e));
	});

	$effect(() => {
		const f = filters;
		items = [];
		next = null;
		opening = null;
		fetchPage(f, null);
	});

	const ref = reference();
	const amounts = (tx: Schemas['Transaction'], r: Reference) =>
		filters.account_id ? effectOn(tx, filters.account_id, r) : size(tx, r);

	/** A form under a row closed: the row shows what was saved, in its place. */
	function closed(id: string, change?: { saved?: Schemas['Transaction']; deleted?: true }) {
		open[id] = false;
		if (change?.saved) {
			const tx = change.saved;
			items = items.map((x) => (x.id === id ? tx : x));
		}
		if (change?.deleted) items = items.filter((x) => x.id !== id);
	}

	/** The form at the top closed: a saved transaction goes first, until a reload. */
	function added(change?: { saved?: Schemas['Transaction'] }) {
		adding = null;
		if (change?.saved) items = [change.saved, ...items];
	}

	async function copyAsNew(id: string) {
		adding = null;
		await tick();
		adding = { copy: id };
		await tick();
		top?.scrollIntoView({ block: 'start', behavior: 'smooth' });
	}
</script>

<div class="heading">
	<h1>Transactions</h1>
	<button type="button" disabled={!!adding} onclick={() => (adding = {})}>Add transaction</button>
</div>

{#await ref}
	<p class="muted">Loading…</p>
{:then r}
	{@const running =
		history && opening && filters.account_id
			? runningBalances(items, filters.account_id, opening, r)
			: null}
	<form class="filters" onsubmit={(e) => e.preventDefault()}>
		<input type="search" placeholder="Search text" bind:value={text} oninput={typed} />
		<label
			>From <input
				type="date"
				value={filters.from ?? ''}
				onchange={(e) => set('from', e.currentTarget.value)}
			/></label
		>
		<label
			>To <input
				type="date"
				value={filters.to ?? ''}
				onchange={(e) => set('to', e.currentTarget.value)}
			/></label
		>
		<select
			value={filters.account_id ?? ''}
			onchange={(e) => set('account_id', e.currentTarget.value)}
		>
			<option value="">Any account</option>
			{#each accountsByOwner(r) as a (a.id)}<option value={a.id}>{accountLabel(a, r)}</option
				>{/each}
		</select>
		<select
			value={filters.counterparty_id ?? ''}
			onchange={(e) => set('counterparty_id', e.currentTarget.value)}
		>
			<option value="">Any counterparty</option>
			{#each sorted(r.counterparties) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
		</select>
		<select
			value={filters.category_id ?? ''}
			onchange={(e) => set('category_id', e.currentTarget.value)}
		>
			<option value="">Any category</option>
			{#each sorted(r.categories) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
		</select>
		<select value={filters.tag_id ?? ''} onchange={(e) => set('tag_id', e.currentTarget.value)}>
			<option value="">Any tag</option>
			{#each sorted(r.tags) as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
		</select>
		{#if Object.keys(filters).length}
			<a
				class="clear"
				href={resolve('/transactions')}
				onclick={() => {
					text = '';
				}}>Clear</a
			>
		{/if}
	</form>

	{#if only}
		<p class="batch">
			One transaction
			<button type="button" class="quiet" onclick={() => goto(resolve('/transactions'))}
				>Show all</button
			>
		</p>
	{:else if filters.import_batch_id}
		<p class="batch">
			Import batch
			{#if batch}
				<strong>{batchName(batch)}</strong>
				<span class="muted"
					>created {localDate(batch.created_at)} · {batch.transaction_count}
					{batch.transaction_count === 1 ? 'transaction' : 'transactions'}</span
				>
			{/if}
			<button type="button" class="quiet" onclick={() => set('import_batch_id', '')}
				>Show all</button
			>
		</p>
	{/if}

	<table class="list">
		<thead>
			<tr>
				<th class="date">Date</th>
				<th>Description</th>
				<th>Route</th>
				<th>Category</th>
				<th class="num">{filters.account_id ? 'In / out' : 'Amount'}</th>
				{#if running}<th class="num">Balance</th>{/if}
			</tr>
		</thead>
		<tbody>
			{#if adding}
				<tr class="expanded adding" bind:this={top}>
					<td colspan={running ? 6 : 5}>
						<TransactionForm
							id={null}
							copy={adding.copy}
							account={filters.account_id}
							onclose={added}
						/>
					</td>
				</tr>
			{/if}
			{#each items as tx, i (tx.id)}
				<tr class="row" class:open={open[tx.id]} onclick={() => (open[tx.id] = !open[tx.id])}>
					<td class="date">{tx.date}</td>
					<td class="desc">
						<button class="link" aria-expanded={!!open[tx.id]}
							>{tx.description || '(no description)'}</button
						>
						{#each tx.tags as t (t)}<span class="tag">{t}</span>{/each}
						{#if tx.notes && tx.notes !== tx.description}
							<div class="muted notes">{tx.notes}</div>
						{/if}
					</td>
					<td class="muted route">{route(tx.splits, r)}</td>
					<td class="muted category">{categoryNames(tx.splits, r).join(', ')}</td>
					<td class="num amount"><Quantities items={amounts(tx, r)} /></td>
					{#if running}<td class="num balance"><Quantities items={running[i]} /></td>{/if}
				</tr>
				{#if open[tx.id]}
					<tr class="expanded">
						<td colspan={running ? 6 : 5}>
							<TransactionForm
								id={tx.id}
								onclose={(change) => closed(tx.id, change)}
								oncopy={copyAsNew}
							/>
						</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>

	{#if failure}
		<Failure error={failure} />
	{:else if loading}
		<p class="muted">Loading…</p>
	{:else if items.length === 0 && !adding}
		<p class="muted">No transactions match.</p>
	{:else if next}
		<p><button class="quiet" onclick={() => fetchPage(filters, next)}>Load more</button></p>
	{:else}
		<p class="muted">{items.length} shown, the end.</p>
	{/if}
{:catch error}
	<Failure {error} />
{/await}

<style>
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-4);
	}
	.batch {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2) var(--space-3);
	}
	.filters {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.filters label {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.filters input[type='search'] {
		min-width: 16rem;
	}
	.clear {
		font-size: var(--text-sm);
	}
	.list {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.date {
		white-space: nowrap;
		font-variant-numeric: var(--numeric);
		width: 7rem;
	}
	.row {
		cursor: pointer;
	}
	.row:hover td,
	.row.open td {
		background: var(--color-surface-alt);
	}
	.expanded td {
		border-top: none;
		padding-top: 0;
	}
	.adding td {
		padding-top: var(--space-3);
	}
	button.link {
		padding: 0;
		border: none;
		background: none;
		color: var(--color-text);
		text-align: left;
		cursor: pointer;
	}
	.tag {
		margin-left: var(--space-2);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		background: var(--color-surface-alt);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
	.notes {
		font-size: var(--text-xs);
	}
	/* A phone: each transaction a block, what it is and how much on top. */
	@media (max-width: 40rem) {
		.list,
		.list tbody,
		.list tr,
		.list td {
			display: block;
		}
		.list thead {
			display: none;
		}
		.list tr.row {
			display: grid;
			grid-template-columns: minmax(0, 1fr) auto;
			grid-template-areas:
				'desc amount'
				'date balance'
				'route route'
				'category category';
			column-gap: var(--space-3);
			padding: var(--space-2) var(--space-3);
			border-top: var(--border-width) solid var(--color-border);
		}
		.list tr.row td {
			padding: 0;
			border: none;
		}
		.desc {
			grid-area: desc;
		}
		.amount {
			grid-area: amount;
		}
		.balance {
			grid-area: balance;
			font-size: var(--text-sm);
		}
		.date,
		.route,
		.category {
			width: auto;
			font-size: var(--text-sm);
		}
		.date {
			grid-area: date;
			color: var(--color-text-muted);
		}
		.route {
			grid-area: route;
		}
		.category {
			grid-area: category;
		}
		.list tr.expanded td {
			padding: 0 var(--space-2) var(--space-2);
		}
	}
</style>
