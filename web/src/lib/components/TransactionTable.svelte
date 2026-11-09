<script lang="ts">
	// Transactions as a section (docs/ux.md §5): one `TransactionRow` each, a
	// heading with the total, Load more. Given a filter, it fetches them, and
	// each amount is what the filter counted of the transaction (`matched`),
	// the same amount the report or statement with that filter added up; the
	// total is that figure. Given items, it only draws them. Edit opens the
	// transaction's form under its row (§7), which holds Copy as new.
	import { tick, untrack, type Snippet } from 'svelte';
	import { api, ok, type Schemas } from '../api/client';
	import type { Filter } from '../filters';
	import { reference, type Reference } from '../reference';
	import { categoryNames, quantities, runningBalances } from '../views/transactions';
	import Failure from './Failure.svelte';
	import LegName from './LegName.svelte';
	import Quantities from './Quantities.svelte';
	import Section from './Section.svelte';
	import TransactionForm from './TransactionForm.svelte';
	import TransactionRow from './TransactionRow.svelte';
	import Values from './Values.svelte';
	import { dayName } from '../dates';

	type Transaction = Schemas['Transaction'];
	const PAGE = 100;

	let {
		filter,
		items: given,
		label,
		balance = false,
		open: opened,
		empty = 'No transactions match.',
		values: more,
		oncopy,
		onchange
	}: {
		/** Fetch what this filter matches. */
		filter?: Filter;
		/** Or draw these. */
		items?: Transaction[];
		/** The heading, before the total. */
		label: string;
		/** One account's whole history (`filter` is that account and dates): a running balance. */
		balance?: boolean;
		/** A transaction to show with its form open. */
		open?: string;
		empty?: string;
		/** More values for a row, after its own (`<dt>`, `<dd>`, `<dd class="aside">`). */
		values?: Snippet<[Transaction]>;
		/** Copy as new, asked from a transaction's form: the page opens a form for it. */
		oncopy?: (id: string) => void;
		/** A transaction was saved or deleted. */
		onchange?: () => void;
	} = $props();

	let fetched: Transaction[] = $state([]);
	let totals: Schemas['CommodityAmount'][] | null = $state(null);
	let next: string | null = $state(null);
	let loading = $state(false);
	let failure: unknown = $state(null);
	let opening: Map<string, string> | null = $state(null);
	let editing: Record<string, boolean> = $state(untrack(() => (opened ? { [opened]: true } : {})));
	let generation = 0;

	let items = $derived(given ?? fetched);

	/** Fetches the first page, or the next one, or again as many as are shown. */
	async function load(f: Filter, cursor: string | null, limit = PAGE) {
		const mine = cursor ? generation : ++generation;
		loading = true;
		failure = null;
		try {
			const r = ok(
				await api.GET('/api/v1/transactions', {
					params: { query: { ...f, limit, cursor: cursor ?? undefined } }
				})
			);
			if (mine !== generation) return; // the filter changed meanwhile
			const account = f.account_id?.[0];
			if (!cursor && balance && account && r.items.length) {
				const b = ok(
					await api.GET('/api/v1/reports/balances', {
						params: { query: { date: r.items[0].date } }
					})
				);
				if (mine !== generation) return;
				opening = new Map(
					b.balances.filter((x) => x.account_id === account).map((x) => [x.commodity_id, x.balance])
				);
			}
			fetched = cursor ? [...fetched, ...r.items] : r.items;
			totals = r.totals;
			next = r.next_cursor ?? null;
		} catch (e) {
			if (mine === generation) failure = e;
		} finally {
			if (mine === generation) loading = false;
		}
	}

	$effect(() => {
		const f = filter;
		if (!f) return;
		untrack(() => {
			fetched = [];
			totals = null;
			next = null;
			opening = null;
			load(f, null);
		});
	});

	/** A form closed: what changed may change what counts, so the shown rows are fetched again. */
	function closed(id: string, change?: { saved?: Transaction; deleted?: true }) {
		editing[id] = false;
		if (!change) return;
		onchange?.();
		if (filter) load(filter, null, Math.min(Math.max(fetched.length, PAGE), 500));
		else if (change.deleted) given = given?.filter((x) => x.id !== id);
		else if (change.saved) {
			const tx = change.saved;
			given = given?.map((x) => (x.id === id ? tx : x));
		}
	}

	async function copy(id: string) {
		await tick();
		oncopy?.(id);
	}
</script>

{#await reference()}
	<p class="muted">Loading…</p>
{:then r}
	{@const running = balance && opening ? runningBalances(items, opening, r) : null}
	<Section {label} tag="section">
		<div class="table">
			<h2 class="heading">
				<span>{label}</span>
				{#if totals}
					<span class="total"><Quantities items={quantities(totals, r)} inline /></span>
				{/if}
			</h2>
			{#each items as tx, i (tx.id)}
				{@render row(tx, r, running?.[i])}
			{:else}
				{#if !loading && !failure}<p class="muted none">{empty}</p>{/if}
			{/each}
			{#if failure}
				<div class="foot"><Failure error={failure} /></div>
			{:else if loading}
				<p class="muted foot">Loading…</p>
			{:else if next && filter}
				{@const f = filter}
				<p class="foot">
					<button class="quiet" onclick={() => load(f, next)}>Load more</button>
				</p>
			{/if}
		</div>
	</Section>
{:catch error}
	<Failure {error} />
{/await}

{#snippet row(
	tx: Transaction,
	r: Reference,
	after: ReturnType<typeof runningBalances>[number] | undefined
)}
	{@const categories = categoryNames(tx.splits, r)}
	{#snippet description()}
		<div>{tx.description}</div>
		{#each tx.tags as t (t)}<span class="tag">{t}</span>{/each}
		{#if tx.notes && tx.notes !== tx.description}<div class="muted notes">{tx.notes}</div>{/if}
	{/snippet}
	{#snippet amount()}<Quantities items={quantities(tx.matched, r)} />{/snippet}
	{#snippet values()}
		<Values>
			{#if tx.splits.length === 1}
				<dt>From</dt>
				<dd><LegName leg={tx.splits[0].from} ref={r} /></dd>
				<dd class="aside"></dd>
				<dt>To</dt>
				<dd><LegName leg={tx.splits[0].to} ref={r} /></dd>
				<dd class="aside"></dd>
			{:else}
				<dt>Splits</dt>
				<dd>{tx.splits.length}</dd>
				<dd class="aside"></dd>
			{/if}
			{#if categories.length}
				<dt>{categories.length === 1 ? 'Category' : 'Categories'}</dt>
				<dd>{categories.join(', ')}</dd>
				<dd class="aside"></dd>
			{/if}
			{#if after}
				<dt>Balance</dt>
				<dd><Quantities items={after} inline /></dd>
				<dd class="aside"></dd>
			{/if}
			{@render more?.(tx)}
		</Values>
	{/snippet}
	{#snippet actions()}
		<button
			class="box"
			aria-expanded={!!editing[tx.id]}
			onclick={() => (editing[tx.id] = !editing[tx.id])}>Edit</button
		>
	{/snippet}
	{#snippet form()}
		<TransactionForm
			id={tx.id}
			onclose={(change) => closed(tx.id, change)}
			oncopy={oncopy ? copy : undefined}
		/>
	{/snippet}
	<TransactionRow
		label="{dayName(tx.date)} {tx.description}"
		date={tx.date}
		{description}
		{amount}
		{values}
		{actions}
		form={editing[tx.id] ? form : undefined}
	/>
{/snippet}

<style>
	.table {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.heading {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-3);
		margin: 0;
		padding: var(--space-2) var(--space-3) var(--space-2) calc(var(--space-3) + 4px);
		border-bottom: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md) var(--radius-md) 0 0;
		background: var(--color-surface-alt);
		font-size: var(--text-base);
	}
	.total {
		white-space: nowrap;
	}
	.none,
	.foot {
		margin: 0;
		padding: var(--space-3);
	}
	.foot {
		border-top: var(--border-width) solid var(--color-border);
	}
	.tag {
		margin-right: var(--space-2);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		background: var(--color-surface-alt);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
	.notes {
		font-size: var(--text-xs);
	}
</style>
