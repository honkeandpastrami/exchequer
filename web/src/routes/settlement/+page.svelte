<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import StatusMark from '#lib/components/StatusMark.svelte';
	import { monthName, thisMonth } from '#lib/dates.ts';
	import { hashQuery } from '#lib/query.ts';
	import { reference } from '#lib/reference.ts';
	import { byEdge } from '#lib/views/settlement.ts';

	const current = thisMonth();
	let year = $derived(hashQuery(page.url).get('year') ?? current.slice(0, 4));

	async function load(year: string) {
		const to = year === current.slice(0, 4) ? current : `${year}-12`;
		const [ref, history] = await Promise.all([
			reference(),
			api.GET('/api/v1/settlements', { params: { query: { from: `${year}-01`, to } } }).then(ok)
		]);
		return { ref, edges: byEdge(history, ref.parties), unpriced: history.unpriced };
	}

	let data = $derived(load(year));
	const go = (y: number) => goto(resolve(`/settlement?year=${y}`), { replace: true });

	const columns = [
		['carried', 'Carried'],
		['claims', 'Claims'],
		['stipend', 'Stipend'],
		['due', 'Due'],
		['paid', 'Paid'],
		['outstanding', 'Outstanding'],
		['to_settle', 'To settle']
	] as const;
</script>

<div class="heading">
	<h1>Settlement</h1>
	<div class="year">
		<button class="quiet" onclick={() => go(+year - 1)} aria-label="Previous year">←</button>
		<strong>{year}</strong>
		<button
			class="quiet"
			onclick={() => go(+year + 1)}
			disabled={year >= current.slice(0, 4)}
			aria-label="Next year">→</button
		>
	</div>
</div>
<p class="muted legend">
	Positive amounts go from the first party to the second.
	<code>outstanding = carried + due − paid</code>; <code>due = claims − stipend</code>.
</p>

{#await data}
	<p class="muted">Loading…</p>
{:then d}
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
			{d.unpriced.length} splits have no value in the household currency and are left out.
		</p>
	{/if}
{:catch error}
	<Failure {error} />
{/await}

<style>
	.heading {
		display: flex;
		align-items: baseline;
		gap: var(--space-5);
	}
	.year {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.year button {
		padding: var(--space-1) var(--space-3);
	}
	.legend {
		font-size: var(--text-sm);
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
