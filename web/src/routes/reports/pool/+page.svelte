<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import BarList from '#lib/charts/BarList.svelte';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import { addMonths, lastDay, monthName, thisMonth } from '#lib/dates.ts';
	import { minus, negate, sign, sum, zero } from '#lib/money.ts';
	import { hashQuery } from '#lib/query.ts';
	import { reference, type Reference } from '#lib/reference.ts';

	let month = $derived(
		hashQuery(page.url)
			.get('month')
			?.match(/^\d{4}-\d{2}$/)?.[0] ?? thisMonth()
	);

	async function load(month: string) {
		const [ref, pool] = await Promise.all([
			reference(),
			api.GET('/api/v1/reports/household-pool/{month}', { params: { path: { month } } }).then(ok)
		]);
		return { ref, pool };
	}

	let data = $derived(load(month));

	const at = (m: string) => resolve(`/reports/pool?month=${m}`);
	const name = (r: Reference, id: string | null | undefined) =>
		id ? (r.categories.get(id)?.name ?? '?') : 'Uncategorized';
	const link = (id: string | null | undefined) =>
		id
			? resolve(`/transactions?category_id=${id}&from=${month}-01&to=${lastDay(month)}`)
			: undefined;

	function bars(r: Reference, lines: Schemas['CategoryAmount'][], flip: boolean) {
		return lines
			.map((l) => ({
				key: l.category_id ?? '',
				label: name(r, l.category_id),
				value: flip ? negate(l.amount) : l.amount,
				href: link(l.category_id)
			}))
			.sort((a, b) => sign(minus(b.value, a.value)));
	}
</script>

<div class="heading">
	<h1>Household pool, {monthName(month)}</h1>
	<nav class="months">
		<a href={at(addMonths(month, -1))}>← {monthName(addMonths(month, -1))}</a>
		{#if month < thisMonth()}
			<a href={at(addMonths(month, 1))}>{monthName(addMonths(month, 1))} →</a>
		{/if}
	</nav>
</div>

{#await data}
	<p class="muted">Loading…</p>
{:then { ref, pool }}
	{@const currency = ref.commodities.get(pool.currency_id)}
	{@const code = currency?.code}
	{@const none = zero(currency?.precision ?? 0)}
	{@const income = sum([none, ...pool.income.map((l) => l.amount)])}
	{@const spent = negate(sum([none, ...pool.expenses.map((l) => l.amount)]))}
	<section class="totals">
		<div class="card stat">
			<span class="muted">Shared income</span>
			<Amount value={income} {code} />
		</div>
		<div class="card stat">
			<span class="muted">Shared spending</span>
			<Amount value={spent} {code} />
		</div>
		<div class="card stat">
			<span class="muted">Change in shared savings</span>
			<Amount value={pool.shared_savings_change} {code} />
		</div>
	</section>

	<div class="columns">
		<section class="card">
			<h2>Income</h2>
			{#if pool.income.length}
				<BarList bars={bars(ref, pool.income, false)} />
			{:else}
				<p class="muted">None.</p>
			{/if}
		</section>
		<section class="card">
			<h2>Spending</h2>
			{#if pool.expenses.length}
				<BarList bars={bars(ref, pool.expenses, true)} />
			{:else}
				<p class="muted">None.</p>
			{/if}
		</section>
	</div>

	<div class="columns">
		<section class="card">
			<h2>Stipends</h2>
			<p class="muted hint">What the household owes each person for the month.</p>
			{#each pool.stipends as s (s.party_id)}
				<p class="line">
					<span>{ref.parties.get(s.party_id)?.name ?? '?'}</span>
					<Amount value={s.amount} />
				</p>
			{:else}
				<p class="muted">None set.</p>
			{/each}
		</section>
		<section class="card">
			<h2>Business funding</h2>
			<p class="muted hint">How much each business's net funding grew this month.</p>
			{#each pool.business_funding as b (b.party_id)}
				<p class="line">
					<span>{ref.parties.get(b.party_id)?.name ?? '?'}</span>
					<Amount value={b.amount} />
				</p>
			{:else}
				<p class="muted">None.</p>
			{/each}
		</section>
	</div>
	<p>
		<a href={resolve('/settlement/[month]', { month })}>Settlement statement</a>
	</p>
{:catch error}
	<Failure {error} />
{/await}

<style>
	.heading {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2) var(--space-5);
	}
	.months {
		display: flex;
		gap: var(--space-4);
		font-size: var(--text-sm);
	}
	.card {
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.totals,
	.columns {
		display: grid;
		gap: var(--space-4);
		margin-bottom: var(--space-4);
	}
	.totals {
		grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
	}
	.columns {
		grid-template-columns: repeat(auto-fit, minmax(20rem, 1fr));
	}
	.stat {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-lg);
	}
	.stat .muted {
		font-size: var(--text-sm);
	}
	.line {
		display: flex;
		justify-content: space-between;
		gap: var(--space-3);
		margin: var(--space-2) 0 0;
		font-size: var(--text-sm);
	}
	.hint {
		margin-top: 0;
		font-size: var(--text-sm);
	}
</style>
