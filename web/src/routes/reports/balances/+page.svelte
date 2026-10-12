<script lang="ts">
	import { usd } from '#lib/usd.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Chart from '#lib/charts/Chart.svelte';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import MonthWindow from '#lib/components/MonthWindow.svelte';
	import { lastDay, monthName, thisMonth, today } from '#lib/dates.ts';
	import { hashQuery, withParam } from '#lib/query.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { reference, sorted, type Reference } from '#lib/reference.ts';
	import { balanceSeries, buckets, commoditiesIn } from '#lib/views/reports.ts';
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
		account: string
	) {
		const [ref, result] = await Promise.all([
			reference(),
			api
				.POST('/api/v1/query', {
					body: {
						measure: 'balance',
						from: `${start}-01`,
						to: end === thisMonth() ? today() : lastDay(end),
						bucket,
						group_by: [by === 'owner_id' ? 'owner' : 'account'],
						owner_ids: owner ? [owner] : [],
						account_ids: account ? [account] : []
					}
				})
				.then(ok)
		]);
		return { ref, rows: result.rows };
	}

	let data = $derived(load(w.start, w.end, bucket, by, owner, account));

	const household = (r: Reference) => usd(r)?.id;
	const names = (r: Reference) =>
		new Map<string, string>([
			...[...r.accounts.values()].map((a) => [a.id, accountLabel(a, r)] as const),
			...[...r.parties.values()].map((p) => [p.id, p.name] as const)
		]);
	const label = (b: string) => (bucket === 'month' ? monthName(b.slice(0, 7)) : b);

	function view(rows: Schemas['Row'][], r: Reference, wanted: string | null) {
		const commodities = commoditiesIn(rows, household(r));
		const commodity = wanted && commodities.includes(wanted) ? wanted : commodities[0];
		const at = buckets(rows);
		const series = commodity ? balanceSeries(rows, by, commodity, names(r), at) : [];
		return { commodities, commodity, at, series };
	}
</script>

<div class="heading">
	<h1>Balance history</h1>
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
			value={by === 'owner_id' ? 'owner' : 'account'}
			onchange={(e) => set({ by: e.currentTarget.value === 'account' ? 'account' : '' })}
			aria-label="One line per"
		>
			<option value="owner">By owner</option>
			<option value="account">By account</option>
		</select>
		<select value={owner} onchange={(e) => set({ owner: e.currentTarget.value })}>
			<option value="">Every owner</option>
			{#each sorted(d.ref.parties) as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
		</select>
		<select value={account} onchange={(e) => set({ account: e.currentTarget.value })}>
			<option value="">Every account</option>
			{#each accountsByOwner(d.ref) as a (a.id)}<option value={a.id}
					>{accountLabel(a, d.ref)}</option
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
	</form>

	{#if v.series.length}
		<section class="card">
			<Chart labels={v.at.map(label)} series={v.series} {code} />
		</section>
		<section class="card table">
			<table>
				<thead>
					<tr>
						<th>{bucket === 'month' ? 'Month end' : bucket === 'week' ? 'Week of' : 'Day'}</th>
						{#each v.series as s (s.name)}<th class="num">{s.name}</th>{/each}
					</tr>
				</thead>
				<tbody>
					{#each v.at.toReversed() as b, ri (b)}
						{@const i = v.at.length - 1 - ri}
						<tr>
							<td>{label(b)}</td>
							{#each v.series as s (s.name)}
								<td class="num" class:total={s.name === 'Total'}><Amount value={s.values[i]} /></td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
		<p class="muted note">
			Balances at the end of each {bucket}, in {code ?? 'one commodity'} only; other commodities aren't
			converted. Credit cards and loans count as negative.
		</p>
	{:else}
		<p class="muted">No balances in this range.</p>
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
	.table {
		overflow-x: auto;
		max-height: 32rem;
		overflow-y: auto;
		padding: 0;
	}
	.total {
		font-weight: var(--weight-strong);
	}
	.note {
		font-size: var(--text-sm);
	}
</style>
