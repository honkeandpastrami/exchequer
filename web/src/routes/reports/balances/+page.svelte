<script lang="ts">
	import { usd } from '#lib/settlement-currency.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Chart from '#lib/charts/Chart.svelte';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import MonthRange from '#lib/components/dates/MonthRange.svelte';
	import OneRow from '#lib/components/OneRow.svelte';
	import PageHeading from '#lib/components/PageHeading.svelte';
	import Section from '#lib/components/Section.svelte';
	import { transactionsPath, type Filter } from '#lib/filters.ts';
	import { dayName, lastDay, monthName, thisMonth, today } from '#lib/dates.ts';
	import { hashQuery, withParam } from '#lib/query.ts';
	import { holding, holdings, type Holding } from '#lib/labels.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { reference, sorted, type Reference } from '#lib/reference.ts';
	import { balanceSeries, bucketEnd, buckets, commoditiesIn } from '#lib/views/reports.ts';
	import { readWindow } from '#lib/views/window.ts';

	type Bucket = 'month' | 'week' | 'day';
	type By = 'owner_id' | 'account_id';

	let query = $derived(hashQuery(page.url));
	let w = $derived(readWindow(query));
	let bucket: Bucket = $derived(
		(['week', 'day'] as const).find((b) => b === query.get('bucket')) ?? 'month'
	);
	let by: By = $derived(query.get('by') === 'account' ? 'account_id' : 'owner_id');
	let owner = $derived(query.get('owner') ?? '');
	let account = $derived(query.get('account') ?? '');
	// Liquid unless asked: loans are large and slow, and would flatten the rest.
	let held: Holding | 'all' = $derived(
		(['investments', 'loans', 'all'] as const).find((h) => h === query.get('holding')) ?? 'liquid'
	);
	const inHolding = (a: Schemas['Account'], h: Holding | 'all') => h === 'all' || holding(a) === h;

	function set(changes: Record<string, string>) {
		let q = query;
		for (const [k, v] of Object.entries(changes)) q = new URLSearchParams(withParam(q, k, v));
		goto(resolve(q.size ? `/reports/balances?${q}` : '/reports/balances'), {
			replace: true,
			reset: false
		});
	}

	async function load(
		start: string,
		end: string,
		bucket: Bucket,
		by: By,
		owner: string,
		account: string,
		held: Holding | 'all'
	) {
		const ref = await reference();
		const accounts = account
			? [account]
			: held === 'all'
				? []
				: [...ref.accounts.values()].filter((a) => inHolding(a, held)).map((a) => a.id);
		// No accounts in the holding: nothing to ask for (an empty filter means every account).
		if (held !== 'all' && accounts.length === 0) return { ref, rows: [], accounts };
		const result = await api
			.POST('/api/v1/query', {
				body: {
					measure: 'balance',
					from: `${start}-01`,
					to: last(end),
					bucket,
					group_by: [by === 'owner_id' ? 'owner' : 'account'],
					owner_ids: owner ? [owner] : [],
					account_ids: accounts
				}
			})
			.then(ok);
		return { ref, rows: result.rows, accounts };
	}

	/** The range's last day: today, in the current month. */
	const last = (end: string) => (end === thisMonth() ? today() : lastDay(end));

	let data = $derived(load(w.start, w.end, bucket, by, owner, account, held));

	const household = (r: Reference) => usd(r)?.id;
	const names = (r: Reference) =>
		new Map<string, string>([
			...[...r.accounts.values()].map((a) => [a.id, accountLabel(a, r)] as const),
			...[...r.parties.values()].map((p) => [p.id, p.name] as const)
		]);
	const label = (b: string) =>
		bucket === 'month' ? monthName(b.slice(0, 7)) : dayName(b.slice(0, 10));

	function view(rows: Schemas['Row'][], r: Reference, wanted: string | null) {
		const commodities = commoditiesIn(rows, household(r));
		const commodity = wanted && commodities.includes(wanted) ? wanted : commodities[0];
		const at = buckets(rows);
		const total = held === 'all' ? 'Net' : `${holdings[held]} total`;
		const series = commodity ? balanceSeries(rows, by, commodity, names(r), at, total) : [];
		return { commodities, commodity, at, series };
	}
</script>

<PageHeading>
	<h1>Balance history</h1>
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
			value={by === 'owner_id' ? 'owner' : 'account'}
			onchange={(e) => set({ by: e.currentTarget.value === 'account' ? 'account' : '' })}
			aria-label="One line per"
		>
			<option value="owner">By owner</option>
			<option value="account">By account</option>
		</select>
		<select
			value={held}
			onchange={(e) => {
				const h = e.currentTarget.value;
				const a = d.ref.accounts.get(account);
				// The account picked stays only if it's in the new holding.
				const keep = a && (h === 'all' || holding(a) === h);
				set({ holding: h === 'liquid' ? '' : h, account: keep ? account : '' });
			}}
			aria-label="Holding"
		>
			{#each Object.entries(holdings) as [h, name] (h)}<option value={h}>{name}</option>{/each}
			<option value="all">All (net)</option>
		</select>
		<select value={owner} onchange={(e) => set({ owner: e.currentTarget.value })}>
			<option value="">Every owner</option>
			{#each sorted(d.ref.parties) as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
		</select>
		<select value={account} onchange={(e) => set({ account: e.currentTarget.value })}>
			<option value="">Every account</option>
			{#each accountsByOwner(d.ref).filter((a) => inHolding(a, held)) as a (a.id)}<option
					value={a.id}>{accountLabel(a, d.ref)}</option
				>{/each}
		</select>
		<select
			value={bucket}
			onchange={(e) =>
				set({ bucket: e.currentTarget.value === 'month' ? '' : e.currentTarget.value })}
			aria-label="Points"
		>
			<option value="month">Monthly</option>
			<option value="week">Weekly</option>
			<option value="day">Daily</option>
		</select>
		{#if v.commodities.length > 1}
			<select value={v.commodity} onchange={(e) => set({ commodity: e.currentTarget.value })}>
				{#each v.commodities as c (c)}
					<option value={c}>{d.ref.commodities.get(c)?.code ?? c}</option>
				{/each}
			</select>
		{/if}
	</OneRow>

	{#if v.series.length}
		<section class="card">
			<Chart labels={v.at.map(label)} series={v.series} {code} />
		</section>
		{@const head = bucket === 'month' ? 'Month end' : bucket === 'week' ? 'Week of' : 'Day'}
		{@const isTotal = (s: (typeof v.series)[number]) => !s.key && v.series.length > 1}
		{@const cellLink = (s: (typeof v.series)[number], i: number) => {
			const f: Filter = {
				to: bucketEnd(v.at[i].slice(0, 10), bucket, last(w.end)),
				...(v.commodity ? { commodity_id: [v.commodity] } : {}),
				...(d.accounts.length ? { account_id: d.accounts } : {}),
				...(owner ? { owner_id: [owner] } : {})
			};
			if (s.key && by === 'account_id') f.account_id = [s.key];
			if (s.key && by === 'owner_id') f.owner_id = [s.key];
			return resolve(transactionsPath(f));
		}}
		<Section label="Balances" tag="section">
			{#snippet children(look)}
				{#if look === 'wide'}
					<div class="card table">
						<table>
							<thead>
								<tr>
									<th>{head}</th>
									{#each v.series as s (s.name)}<th class="num">{s.name}</th>{/each}
								</tr>
							</thead>
							<tbody>
								{#each v.at.toReversed() as b, ri (b)}
									{@const i = v.at.length - 1 - ri}
									<tr>
										<td>{label(b)}</td>
										{#each v.series as s (s.name)}
											<td class="num" class:total={isTotal(s)}
												><a href={cellLink(s, i)}><Amount value={s.values[i]} /></a></td
											>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				{:else}
					{#each v.at.toReversed() as b, ri (b)}
						{@const i = v.at.length - 1 - ri}
						<div class="card block">
							<h3>{label(b)}</h3>
							<dl>
								{#each v.series as s (s.name)}
									<dt class:total={isTotal(s)}>{s.name}</dt>
									<dd class:total={isTotal(s)}>
										<a href={cellLink(s, i)}><Amount value={s.values[i]} /></a>
									</dd>
								{/each}
							</dl>
						</div>
					{/each}
				{/if}
			{/snippet}
		</Section>
	{:else}
		<p class="muted">No balances in this range.</p>
	{/if}
{:catch error}
	<Failure {error} />
{/await}

<style>
	.card {
		margin-bottom: var(--space-4);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.table {
		max-height: 32rem;
		overflow-y: auto;
		padding: 0;
	}
	.block h3 {
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
	.total {
		font-weight: var(--weight-strong);
	}
</style>
