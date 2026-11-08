<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import StatusMark from '#lib/components/StatusMark.svelte';
	import DateStepper from '#lib/components/dates/DateStepper.svelte';
	import { monthName, thisMonth } from '#lib/dates.ts';
	import { fetched, latest } from '#lib/price-fetching.svelte.ts';
	import { hashQuery } from '#lib/query.ts';
	import { reference } from '#lib/reference.ts';
	import { SETTLEMENT_CODE } from '#lib/settlement-currency.ts';
	import { byEdge } from '#lib/views/settlement.ts';

	const current = thisMonth();
	let year = $derived(hashQuery(page.url).get('year') ?? current.slice(0, 4));

	async function load(year: string) {
		const to = year === current.slice(0, 4) ? current : `${year}-12`;
		const [ref, history] = await Promise.all([
			reference(),
			api
				.GET('/api/v1/settlements', { params: { query: { from: `${year}-01`, to } } })
				.then(ok)
				.then(fetched)
		]);
		return { ref, edges: byEdge(history, ref.parties), unpriced: history.unpriced };
	}

	const data = latest(
		() => year,
		() => load(year)
	);
	const go = (y: string) => goto(resolve(`/settlement?year=${y}`), { replace: true });

	const columns = [
		['carried', 'Carried'],
		['claims', 'Claims'],
		['stipend', 'Stipend'],
		['due', 'Due'],
		['paid', 'Paid'],
		['outstanding', 'Outstanding']
	] as const;
</script>

<div class="heading">
	<h1>Settlement</h1>
	<DateStepper
		value={year}
		granularity="year"
		max={current.slice(0, 4)}
		name="Year"
		onchange={go}
	/>
</div>

{#await data.promise}
	{#if !data.value}<p class="muted">Loading…</p>{/if}
{:catch error}
	<Failure {error} />
{/await}

{#if data.value}
	{@const d = data.value}
	{#each d.edges as e (e.key)}
		<section class="card">
			<h2>{e.child?.name ?? '?'} → {e.parent?.name ?? '?'}</h2>
			<table>
				<thead>
					<tr>
						<th>Month</th>
						{#each columns as [, label] (label)}<th class="num">{label}</th>{/each}
						<th>Status</th>
					</tr>
				</thead>
				<tbody>
					{#each e.months as m (m.month)}
						<tr>
							<td
								><a href={resolve('/settlement/[month]', { month: m.month })}
									>{monthName(m.month)}</a
								></td
							>
							{#each columns as [key] (key)}
								<td class="num"><Amount value={m.edge[key]} /></td>
							{/each}
							<td><StatusMark status={m.status} /></td>
						</tr>
					{/each}
				</tbody>
			</table>
		</section>
	{:else}
		<p class="muted">No settlement edges in {year}.</p>
	{/each}
	{#if d.unpriced.length}
		<p class="muted">
			{d.unpriced.length} splits have no price in {SETTLEMENT_CODE} and are left out.
		</p>
	{/if}
{/if}

<style>
	.heading {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2) var(--space-5);
	}
	.card {
		margin-bottom: var(--space-5);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
		overflow-x: auto;
	}
</style>
