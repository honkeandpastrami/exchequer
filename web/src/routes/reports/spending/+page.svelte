<script lang="ts">
	import { usd } from '#lib/usd.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import BarList from '#lib/charts/BarList.svelte';
	import Chart from '#lib/charts/Chart.svelte';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import MonthWindow from '#lib/components/MonthWindow.svelte';
	import { lastDay, monthName, monthsBetween } from '#lib/dates.ts';
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

	async function load(start: string, end: string, kind: Kind, purpose: string) {
		const [ref, result] = await Promise.all([
			reference(),
			api
				.POST('/api/v1/query', {
					body: {
						from: `${start}-01`,
						to: lastDay(end),
						bucket: 'month',
						group_by: ['category'],
						kinds: [kind],
						purpose_ids: purpose ? [purpose] : []
					}
				})
				.then(ok)
		]);
		return { ref, rows: result.rows };
	}

	let data = $derived(load(w.start, w.end, kind, purpose));
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
	const link = (id: string | null) =>
		id
			? resolve(`/transactions?category_id=${id}&from=${w.start}-01&to=${lastDay(w.end)}`)
			: undefined;
</script>

<div class="heading">
	<h1>{kind === 'expense' ? 'Spending' : 'Income'} by category</h1>
	<MonthWindow
		end={w.end}
		span={w.span}
		onchange={(end, span) => set({ end, span: String(span) })}
	/>
</div>

{#await data}
	<p class="muted">Loading…</p>
{:then d}
	{@const v = view(d.rows, d.ref, query.get('commodity'))}
	{@const code = v.commodity ? d.ref.commodities.get(v.commodity)?.code : undefined}
	<form class="controls" onsubmit={(e) => e.preventDefault()}>
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
	</form>

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
			<h2>Total <Amount value={t.total} {code} /></h2>
			<BarList
				{code}
				bars={t.categories.map((c) => ({
					key: c.category ?? '',
					label: name(d.ref, c.category),
					value: c.total,
					href: link(c.category)
				}))}
			/>
		</section>
		<section class="card table">
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
							{#each c.monthly as m, i (i)}<td class="num"><Amount value={m} /></td>{/each}
							<td class="num total"><Amount value={c.total} /></td>
						</tr>
					{/each}
					<tr class="sum">
						<td>Total</td>
						{#each t.monthly as m, i (i)}<td class="num"><Amount value={m} /></td>{/each}
						<td class="num"><Amount value={t.total} /></td>
					</tr>
				</tbody>
			</table>
		</section>
		<p class="muted note">
			{kind === 'expense'
				? 'Money out reads positive; refunds reduce it.'
				: 'Money in reads positive.'} In {code ?? 'one commodity'} only. Category links show every transaction
			in the category, whoever it was for.
		</p>
	{:else}
		<p class="muted">Nothing in this range.</p>
	{/if}
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
	.controls {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.card {
		margin-bottom: var(--space-4);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.card h2 {
		display: flex;
		gap: var(--space-3);
		align-items: baseline;
	}
	.table {
		overflow-x: auto;
		padding: 0;
	}
	.name {
		white-space: nowrap;
	}
	.total,
	.sum td {
		font-weight: var(--weight-strong);
	}
	.note {
		font-size: var(--text-sm);
	}
</style>
