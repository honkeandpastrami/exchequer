<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import PageHeading from '#lib/components/PageHeading.svelte';
	import Section from '#lib/components/Section.svelte';
	import StatusMark from '#lib/components/StatusMark.svelte';
	import DateStepper from '#lib/components/dates/DateStepper.svelte';
	import { lastDay, monthName, thisMonth } from '#lib/dates.ts';
	import { fetched, latest } from '#lib/price-fetching.svelte.ts';
	import { transactionsPath } from '#lib/filters.ts';
	import { hashQuery } from '#lib/query.ts';
	import { reference } from '#lib/reference.ts';
	import { SETTLEMENT_CODE } from '#lib/settlement-currency.ts';
	import { byEdge, type EdgeHistory } from '#lib/views/settlement.ts';

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
		return { ref, edges: byEdge(history, ref.parties), unpriced: history.unpriced, to };
	}

	const data = latest(
		() => year,
		() => load(year)
	);
	const go = (y: string) => goto(resolve(`/settlement?year=${y}`), { replace: true });

	/** The figures, and the statement lines a figure adds up, if it does. */
	const columns = [
		['carried', 'Carried', null],
		['claims', 'Claims', 'claim'],
		['stipend', 'Stipend', null],
		['due', 'Due', null],
		['paid', 'Paid', 'payment'],
		['outstanding', 'Outstanding', null]
	] as const;
</script>

<PageHeading>
	<h1>Settlement</h1>
	<DateStepper
		value={year}
		granularity="year"
		max={current.slice(0, 4)}
		name="Year"
		onchange={go}
	/>
</PageHeading>

{#await data.promise}
	{#if !data.value}<p class="muted">Loading…</p>{/if}
{:catch error}
	<Failure {error} />
{/await}

{#if data.value}
	{@const d = data.value}
	{#each d.edges as e (e.key)}
		<Section label="{e.child?.name ?? '?'} → {e.parent?.name ?? '?'}" tag="section">
			{#snippet children(look)}
				<div class="card">
					<h2>{e.child?.name ?? '?'} → {e.parent?.name ?? '?'}</h2>
					{#if look === 'wide'}
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
										{#each columns as [key, , line] (key)}
											<td class="num">{@render figure(m, key, line)}</td>
										{/each}
										<td><StatusMark status={m.status} /></td>
									</tr>
								{/each}
							</tbody>
						</table>
					{:else}
						{#each e.months as m (m.month)}
							<div class="month">
								<h3>
									<a href={resolve('/settlement/[month]', { month: m.month })}
										>{monthName(m.month)}</a
									>
									<StatusMark status={m.status} />
								</h3>
								<dl>
									{#each columns as [key, label, line] (key)}
										<dt>{label}</dt>
										<dd>{@render figure(m, key, line)}</dd>
									{/each}
								</dl>
							</div>
						{/each}
					{/if}
				</div>
			{/snippet}
		</Section>
	{:else}
		<p class="muted">No settlement edges in {year}.</p>
	{/each}
	{#if d.unpriced.length}
		<p>
			<a href={resolve(transactionsPath({ unpriced: true, to: lastDay(d.to) }))}
				>{d.unpriced.length} splits have no price in {SETTLEMENT_CODE} and are left out</a
			>
		</p>
	{/if}
{/if}

{#snippet figure(
	m: EdgeHistory['months'][number],
	key: (typeof columns)[number][0],
	line: 'claim' | 'payment' | null
)}
	{#if line}
		<a href={resolve(transactionsPath({ settles: m.month, party_id: m.edge.child_id, line }))}
			><Amount value={m.edge[key]} /></a
		>
	{:else}
		<Amount value={m.edge[key]} />
	{/if}
{/snippet}

<style>
	.card {
		margin-bottom: var(--space-5);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.month + .month {
		margin-top: var(--space-3);
		padding-top: var(--space-3);
		border-top: var(--border-width) solid var(--color-border);
	}
	.month h3 {
		display: flex;
		justify-content: space-between;
		gap: var(--space-3);
		margin: 0 0 var(--space-2);
		font-size: var(--text-base);
	}
	.month dl {
		display: grid;
		grid-template-columns: minmax(0, 1fr) max-content;
		gap: var(--space-1) var(--space-3);
		margin: 0;
	}
	.month dt {
		color: var(--color-text-muted);
	}
	.month dd {
		margin: 0;
		text-align: right;
	}
</style>
