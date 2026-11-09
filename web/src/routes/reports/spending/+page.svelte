<script lang="ts">
	import { usd } from '#lib/settlement-currency.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import BarList from '#lib/charts/BarList.svelte';
	import Chart from '#lib/charts/Chart.svelte';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import MonthRange from '#lib/components/dates/MonthRange.svelte';
	import OneRow from '#lib/components/OneRow.svelte';
	import PageHeading from '#lib/components/PageHeading.svelte';
	import Section from '#lib/components/Section.svelte';
	import { lastDay, monthName, monthsBetween } from '#lib/dates.ts';
	import { NONE, queryBody, transactionsPath, type Filter } from '#lib/filters.ts';
	import { hashQuery, withParam } from '#lib/query.ts';
	import { reference, sorted, type Reference } from '#lib/reference.ts';
	import { byCategory, commoditiesIn } from '#lib/views/reports.ts';
	import { readWindow } from '#lib/views/window.ts';

	type Kind = 'expense' | 'income';

	let query = $derived(hashQuery(page.url));
	let w = $derived(readWindow(query));
	let kind: Kind = $derived(query.get('kind') === 'income' ? 'income' : 'expense');
	let purpose = $derived(query.get('purpose') ?? '');

	function set(changes: Record<string, string>) {
		let q = query;
		for (const [k, v] of Object.entries(changes)) q = new URLSearchParams(withParam(q, k, v));
		goto(resolve(q.size ? `/reports/spending?${q}` : '/reports/spending'), {
			replace: true,
			reset: false
		});
	}

	// What every figure is computed from; a figure adds its category and month.
	let filter: Filter = $derived({
		from: `${w.start}-01`,
		to: lastDay(w.end),
		kind: [kind],
		...(purpose ? { purpose_id: [purpose] } : {})
	});

	async function load(filter: Filter) {
		const [ref, result] = await Promise.all([
			reference(),
			api
				.POST('/api/v1/query', {
					body: { ...queryBody(filter, () => undefined), bucket: 'month', group_by: ['category'] }
				})
				.then(ok)
		]);
		return { ref, rows: result.rows };
	}

	let data = $derived(load(filter));
	let months = $derived(monthsBetween(w.start, w.end));

	const household = (r: Reference) => usd(r)?.id;

	function view(rows: Schemas['Row'][], r: Reference, wanted: string | null) {
		const commodities = commoditiesIn(rows, household(r));
		const commodity = wanted && commodities.includes(wanted) ? wanted : commodities[0];
		const totals = commodity ? byCategory(rows, commodity, months, kind === 'expense') : null;
		return { commodities, commodity, totals };
	}

	const name = (r: Reference, id: string | null) =>
		id ? (r.categories.get(id)?.name ?? '?') : 'Uncategorized';
	/** The transactions behind one figure: a category's (`null`: none), a month's, or both. */
	const link = (commodity: string | undefined, at: { category?: string | null; month?: string }) =>
		resolve(
			transactionsPath({
				...filter,
				...(commodity ? { commodity_id: [commodity] } : {}),
				...(at.category !== undefined ? { category_id: [at.category ?? NONE] } : {}),
				...(at.month ? { from: `${at.month}-01`, to: lastDay(at.month) } : {})
			})
		);
</script>

<PageHeading>
	<h1>{kind === 'expense' ? 'Spending' : 'Income'} by category</h1>
	<MonthRange
		end={w.end}
		span={w.span}
		onchange={(end, span) => set({ end, span: String(span) })}
	/>
</PageHeading>

{#await data}
	<p class="muted">Loading…</p>
{:then d}
	{@const v = view(d.rows, d.ref, query.get('commodity'))}
	{@const code = v.commodity ? d.ref.commodities.get(v.commodity)?.code : undefined}
	<OneRow label="Report controls">
		<select
			value={kind}
			onchange={(e) => set({ kind: e.currentTarget.value === 'income' ? 'income' : '' })}
			aria-label="Kind"
		>
			<option value="expense">Spending</option>
			<option value="income">Income</option>
		</select>
		<select value={purpose} onchange={(e) => set({ purpose: e.currentTarget.value })}>
			<option value="">For anyone</option>
			{#each sorted(d.ref.parties) as p (p.id)}<option value={p.id}>For {p.name}</option>{/each}
		</select>
		{#if v.commodities.length > 1}
			<select value={v.commodity} onchange={(e) => set({ commodity: e.currentTarget.value })}>
				{#each v.commodities as c (c)}
					<option value={c}>{d.ref.commodities.get(c)?.code ?? c}</option>
				{/each}
			</select>
		{/if}
	</OneRow>

	{#if v.totals && v.totals.categories.length}
		{@const t = v.totals}
		<section class="card">
			<h2>Per month</h2>
			<Chart
				kind="columns"
				labels={months.map(monthName)}
				series={[{ name: kind === 'expense' ? 'Spending' : 'Income', values: t.monthly }]}
				{code}
			/>
		</section>
		<section class="card">
			<h2>
				Total <a href={link(v.commodity, {})}><Amount value={t.total} {code} /></a>
			</h2>
			<BarList
				{code}
				bars={t.categories.map((c) => ({
					key: c.category ?? '',
					label: name(d.ref, c.category),
					value: c.total,
					href: link(v.commodity, { category: c.category })
				}))}
			/>
		</section>
		{@const cell = (value: string, at: { category?: string | null; month?: string }) => ({
			value,
			href: link(v.commodity, at)
		})}
		<Section label="Per category and month" tag="section">
			{#snippet children(look)}
				{#if look === 'wide'}
					<div class="card table">
						<table>
							<thead>
								<tr>
									<th>Category</th>
									{#each months as m (m)}<th class="num">{monthName(m)}</th>{/each}
									<th class="num">Total</th>
								</tr>
							</thead>
							<tbody>
								{#each t.categories as c (c.category)}
									<tr>
										<td class="name">{name(d.ref, c.category)}</td>
										{#each c.monthly as m, i (i)}
											<td class="num">
												{@render figure(cell(m, { category: c.category, month: months[i] }))}
											</td>
										{/each}
										<td class="num total">
											{@render figure(cell(c.total, { category: c.category }))}
										</td>
									</tr>
								{/each}
								<tr class="sum">
									<td>Total</td>
									{#each t.monthly as m, i (i)}
										<td class="num">{@render figure(cell(m, { month: months[i] }))}</td>
									{/each}
									<td class="num">{@render figure(cell(t.total, {}))}</td>
								</tr>
							</tbody>
						</table>
					</div>
				{:else}
					{#each t.categories as c (c.category)}
						<div class="card block">
							<h3>
								<span>{name(d.ref, c.category)}</span>
								{@render figure(cell(c.total, { category: c.category }))}
							</h3>
							<dl>
								{#each c.monthly as m, i (i)}
									<dt>{monthName(months[i])}</dt>
									<dd>{@render figure(cell(m, { category: c.category, month: months[i] }))}</dd>
								{/each}
							</dl>
						</div>
					{/each}
				{/if}
			{/snippet}
		</Section>
	{:else}
		<p class="muted">Nothing in this range.</p>
	{/if}
{:catch error}
	<Failure {error} />
{/await}

{#snippet figure(f: { value: string; href: string })}
	<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- link() resolves -->
	<a href={f.href}><Amount value={f.value} /></a>
{/snippet}

<style>
	.card {
		margin-bottom: var(--space-4);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.card h2,
	.block h3 {
		display: flex;
		gap: var(--space-3);
		align-items: baseline;
	}
	.block h3 {
		justify-content: space-between;
		margin: 0 0 var(--space-2);
		font-size: var(--text-base);
	}
	.block dl {
		display: grid;
		grid-template-columns: minmax(0, 1fr) max-content;
		gap: var(--space-1) var(--space-3);
		margin: 0;
	}
	.block dt {
		color: var(--color-text-muted);
	}
	.block dd {
		margin: 0;
		text-align: right;
	}
	.table {
		padding: 0;
	}
	.name {
		white-space: nowrap;
	}
	.total,
	.sum td {
		font-weight: var(--weight-strong);
	}
</style>
