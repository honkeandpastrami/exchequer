<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import BarList from '#lib/charts/BarList.svelte';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import PageHeading from '#lib/components/PageHeading.svelte';
	import Section from '#lib/components/Section.svelte';
	import DateStepper from '#lib/components/dates/DateStepper.svelte';
	import { thisMonth } from '#lib/dates.ts';
	import { fetched, latest } from '#lib/price-fetching.svelte.ts';
	import { minus, negate, sign, sum, zero } from '#lib/money.ts';
	import { fromReport, NONE, transactionsPath } from '#lib/filters.ts';
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
			api
				.GET('/api/v1/reports/household-pool/{month}', { params: { path: { month } } })
				.then(ok)
				.then(fetched)
		]);
		return { ref, pool };
	}

	const data = latest(
		() => month,
		() => load(month)
	);

	const at = (m: string) => resolve(`/reports/pool?month=${m}`);
	const name = (r: Reference, id: string | null | undefined) =>
		id ? (r.categories.get(id)?.name ?? '?') : 'Uncategorized';
	type Pool = Schemas['HouseholdPool'];

	/** The transactions behind income or spending. */
	const flows = (pool: Pool, kind: 'income' | 'expense') =>
		resolve(transactionsPath({ ...fromReport(pool.filter), kind: [kind] }));

	function bars(r: Reference, pool: Pool, kind: 'income' | 'expense') {
		const lines = kind === 'income' ? pool.income : pool.expenses;
		return lines
			.map((l) => ({
				key: l.category_id ?? '',
				label: name(r, l.category_id),
				value: kind === 'expense' ? negate(l.amount) : l.amount,
				href: resolve(
					transactionsPath({
						...fromReport(pool.filter),
						kind: [kind],
						category_id: [l.category_id ?? NONE]
					})
				)
			}))
			.sort((a, b) => sign(minus(b.value, a.value)));
	}
</script>

<PageHeading>
	<h1>Household pool</h1>
	<DateStepper
		value={month}
		granularity="month"
		max={thisMonth()}
		name="Month"
		onchange={(m) => goto(at(m))}
	/>
</PageHeading>

{#await data.promise}
	{#if !data.value}<p class="muted">Loading…</p>{/if}
{:catch error}
	<Failure {error} />
{/await}

{#if data.value}
	{@const { ref, pool } = data.value}
	{@const currency = ref.commodities.get(pool.currency_id)}
	{@const code = currency?.code}
	{@const none = zero(currency?.precision ?? 0)}
	{@const income = sum([none, ...pool.income.map((l) => l.amount)])}
	{@const spent = negate(sum([none, ...pool.expenses.map((l) => l.amount)]))}
	<Section label="Totals" tag="section">
		{#snippet children(look)}
			<div class="totals" class:narrow={look === 'narrow'}>
				<div class="card stat">
					<span class="muted">Shared income</span>
					<a href={flows(pool, 'income')}><Amount value={income} {code} /></a>
				</div>
				<div class="card stat">
					<span class="muted">Shared spending</span>
					<a href={flows(pool, 'expense')}><Amount value={spent} {code} /></a>
				</div>
				<div class="card stat">
					<span class="muted">Change in shared savings</span>
					{#if pool.savings_filter}
						<a href={resolve(transactionsPath(fromReport(pool.savings_filter)))}
							><Amount value={pool.shared_savings_change} {code} /></a
						>
					{:else}
						<Amount value={pool.shared_savings_change} {code} />
					{/if}
				</div>
			</div>
		{/snippet}
	</Section>

	<Section label="Income and spending" tag="section">
		{#snippet children(look)}
			<div class="columns" class:narrow={look === 'narrow'}>
				<section class="card">
					<h2>Income</h2>
					{#if pool.income.length}
						<BarList bars={bars(ref, pool, 'income')} />
					{:else}
						<p class="muted">None.</p>
					{/if}
				</section>
				<section class="card">
					<h2>Spending</h2>
					{#if pool.expenses.length}
						<BarList bars={bars(ref, pool, 'expense')} />
					{:else}
						<p class="muted">None.</p>
					{/if}
				</section>
			</div>
		{/snippet}
	</Section>

	<Section label="Stipends and business funding" tag="section">
		{#snippet children(look)}
			<div class="columns" class:narrow={look === 'narrow'}>
				<section class="card">
					<h2>Stipends</h2>
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
		{/snippet}
	</Section>
	<p>
		<a href={resolve('/settlement/[month]', { month })}>Settlement statement</a>
	</p>
{/if}

<style>
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
	/* Wide, side by side, each as wide as its content needs; narrow, one under another. */
	.totals {
		grid-template-columns: repeat(3, minmax(12rem, 1fr));
	}
	.columns {
		grid-template-columns: repeat(2, minmax(20rem, 1fr));
	}
	.totals.narrow,
	.columns.narrow {
		grid-template-columns: minmax(0, 1fr);
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
</style>
