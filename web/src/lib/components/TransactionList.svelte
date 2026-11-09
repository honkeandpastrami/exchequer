<script lang="ts">
	// The Transactions page: the filter bar, what it shows (a chip per
	// filter), and the transactions it matches (`TransactionTable`), or just
	// one (`/transactions/<id>`). Add opens a blank form at the top.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick } from 'svelte';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import Section from '#lib/components/Section.svelte';
	import TransactionForm from '#lib/components/TransactionForm.svelte';
	import TransactionTable from '#lib/components/TransactionTable.svelte';
	import {
		isEmpty,
		parse,
		transactionsPath,
		without,
		withFilter,
		type Filter,
		type ListKey
	} from '#lib/filters.ts';
	import { hashQuery } from '#lib/query.ts';
	import { chips } from '#lib/views/chips.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { reference, sorted } from '#lib/reference.ts';
	import { batchName } from '#lib/views/batches.ts';
	import { stampName } from '#lib/dates.ts';
	import DateField from './dates/DateField.svelte';

	/** Show only this transaction. */
	let { only }: { only: string | null } = $props();

	let query = $derived(hashQuery(page.url));
	let filters: Filter = $derived(parse(query));

	function go(f: Filter) {
		goto(resolve(transactionsPath(f)), { replace: true, reset: false });
	}
	const set = (key: ListKey | 'from' | 'to' | 'q', value: string) =>
		go(withFilter(filters, key, value));
	/** A select's value: the filter's, when it has exactly one. */
	const one = (values: string[] | undefined) => (values?.length === 1 ? values[0] : '');

	// The search box waits for a pause in typing.
	let text = $state(hashQuery(page.url).get('q') ?? '');
	let timer: ReturnType<typeof setTimeout> | undefined;
	function typed() {
		clearTimeout(timer);
		timer = setTimeout(() => set('q', text.trim()), 300);
	}

	/** The form at the top: a new transaction, maybe like an existing one. */
	let adding: { copy?: string } | null = $state(null);
	let top: HTMLElement | undefined = $state();
	/** Bumped when a transaction is added, so the list shows it. */
	let reloads = $state(0);
	// One account's complete history (dates aside): it gets a running balance.
	let history = $derived(
		filters.account_id?.length === 1 &&
			Object.keys(filters).every((k) => ['account_id', 'from', 'to'].includes(k))
	);
	let account = $derived(filters.account_id?.length === 1 ? filters.account_id[0] : undefined);
	let shown: Filter = $derived(only ? { id: [only] } : filters);

	// The batch being shown, when filtering by one.
	let batch: Schemas['ImportBatch'] | null = $state(null);
	let failure: unknown = $state(null);
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

	/** The form at the top closed: a saved transaction shows in the list. */
	function added(change?: { saved?: Schemas['Transaction'] }) {
		adding = null;
		if (change?.saved) reloads++;
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

{#await reference()}
	<p class="muted">Loading…</p>
{:then r}
	<Section label="Filters" tag="form">
		{#snippet children(look)}
			<div class="filters" class:narrow={look === 'narrow'}>
				<input
					class="search"
					type="search"
					placeholder="Search text"
					aria-label="Search text"
					bind:value={text}
					oninput={typed}
				/>
				<span class="bound"
					>From <DateField
						label="From"
						value={filters.from ?? ''}
						onchange={(v) => set('from', v)}
					/></span
				>
				<span class="bound"
					>To <DateField label="To" value={filters.to ?? ''} onchange={(v) => set('to', v)} /></span
				>
				<select
					aria-label="Account"
					value={one(filters.account_id)}
					onchange={(e) => set('account_id', e.currentTarget.value)}
				>
					<option value="">Any account</option>
					{#each accountsByOwner(r) as a (a.id)}<option value={a.id}>{accountLabel(a, r)}</option
						>{/each}
				</select>
				<select
					aria-label="Counterparty"
					value={one(filters.counterparty_id)}
					onchange={(e) => set('counterparty_id', e.currentTarget.value)}
				>
					<option value="">Any counterparty</option>
					{#each sorted(r.counterparties) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
				</select>
				<select
					aria-label="Category"
					value={one(filters.category_id)}
					onchange={(e) => set('category_id', e.currentTarget.value)}
				>
					<option value="">Any category</option>
					{#each sorted(r.categories) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
				</select>
				<select
					aria-label="Tag"
					value={one(filters.tag_id)}
					onchange={(e) => set('tag_id', e.currentTarget.value)}
				>
					<option value="">Any tag</option>
					{#each sorted(r.tags) as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
				</select>
			</div>
		{/snippet}
	</Section>

	{#if only}
		<p class="showing">
			One transaction
			<button type="button" class="quiet" onclick={() => goto(resolve('/transactions'))}
				>Show all</button
			>
		</p>
	{:else if !isEmpty(filters)}
		<Section label="Showing">
			{#snippet children(look)}
				<ul class="showing chips" class:narrow={look === 'narrow'} aria-label="Showing">
					{#each chips(filters, r) as c (`${c.key}=${c.value ?? ''}`)}
						<li>
							<button
								type="button"
								class="chip"
								aria-label="Remove {c.label}"
								onclick={() => {
									if (c.key === 'q') text = '';
									go(without(filters, c.key, c.value));
								}}>{c.label} <span aria-hidden="true">×</span></button
							>
						</li>
					{/each}
					<li>
						<a class="clear" href={resolve(transactionsPath({}))} onclick={() => (text = '')}
							>Clear all</a
						>
					</li>
				</ul>
			{/snippet}
		</Section>
		{#if filters.import_batch_id && batch}
			<p class="showing">
				Import batch <strong>{batchName(batch)}</strong>
				<span class="muted"
					>created {stampName(batch.created_at)} · {batch.transaction_count}
					{batch.transaction_count === 1 ? 'transaction' : 'transactions'}</span
				>
			</p>
		{/if}
	{/if}
	{#if failure}<Failure error={failure} />{/if}

	{#if adding}
		<div class="adding" bind:this={top}>
			<TransactionForm id={null} copy={adding.copy} {account} onclose={added} />
		</div>
	{/if}

	{#key reloads}
		<TransactionTable
			filter={shown}
			label={only ? 'Transaction' : 'Total'}
			balance={!only && history}
			open={only ?? undefined}
			oncopy={copyAsNew}
		/>
	{/key}
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
	/* Wide, two rows: the search and the dates, then the four lists, each
	   column as wide as a list needs. Narrow, one control per line. */
	.filters {
		display: grid;
		grid-template-columns: repeat(4, minmax(10rem, 1fr));
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-3);
	}
	.search {
		grid-column: span 2;
	}
	.filters > :global(select) {
		min-width: 0;
	}
	.filters.narrow {
		grid-template-columns: minmax(0, 1fr);
	}
	.filters.narrow .search {
		grid-column: auto;
	}
	.bound {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		white-space: nowrap;
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.showing {
		margin: 0 0 var(--space-3);
	}
	/* Wide, the chips are one row; narrow, one per line. */
	.chips {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		padding: 0;
		list-style: none;
	}
	.chips > li {
		flex: none;
	}
	.chips.narrow {
		flex-direction: column;
		align-items: start;
	}
	.chip {
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-sm);
		background: var(--color-surface-alt);
		color: var(--color-text);
		font-size: var(--text-sm);
		cursor: pointer;
	}
	.chip:hover {
		border-color: var(--color-accent);
	}
	.clear {
		font-size: var(--text-sm);
	}
	.adding {
		margin-bottom: var(--space-4);
		padding: var(--space-3);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
</style>
