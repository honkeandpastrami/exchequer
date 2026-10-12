<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import StatusMark from '#lib/components/StatusMark.svelte';
	import { addMonths, monthName, thisMonth } from '#lib/dates.ts';
	import { lineKinds } from '#lib/labels.ts';
	import { reference } from '#lib/reference.ts';
	import { transactionsById } from '#lib/transactions.ts';
	import { status } from '#lib/views/settlement.ts';
	import { legName } from '#lib/views/transactions.ts';

	let month = $derived(page.params.month ?? thisMonth());

	async function load(month: string) {
		const [ref, statement, params] = await Promise.all([
			reference(),
			api.GET('/api/v1/settlements/{month}/statement', { params: { path: { month } } }).then(ok),
			api.GET('/api/v1/settlements/{month}', { params: { path: { month } } }).then(ok)
		]);
		const ids = statement.edges.flatMap((e) => [
			...(e.lines ?? []).map((l) => l.transaction_id),
			...(e.in_transit ?? []).map((t) => t.transaction_id)
		]);
		const txs = await transactionsById(ids);
		return { ref, statement, params, txs };
	}

	let data = $derived(load(month));

	const summary = [
		['carried', 'Carried'],
		['claims', 'Claims'],
		['stipend', 'Stipend'],
		['due', 'Due'],
		['paid', 'Paid'],
		['outstanding', 'Outstanding'],
		['to_settle', 'To settle']
	] as const;

	function split(txs: Map<string, Schemas['Transaction']>, p: Schemas['SplitPointer']) {
		const tx = txs.get(p.transaction_id);
		return { tx, split: tx?.splits[p.position] };
	}
</script>

<div class="heading">
	<h1>{monthName(month)}</h1>
	<nav class="months">
		<a href={resolve('/settlement/[month]', { month: addMonths(month, -1) })}
			>← {monthName(addMonths(month, -1))}</a
		>
		<a href={resolve(`/settlement?year=${month.slice(0, 4)}`)}>Year</a>
		<a href={resolve('/settlement/[month]', { month: addMonths(month, 1) })}
			>{monthName(addMonths(month, 1))} →</a
		>
	</nav>
</div>

{#await data}
	<p class="muted">Loading…</p>
{:then { ref, statement, params, txs }}
	{@const name = (id: string) => ref.parties.get(id)?.name ?? '?'}
	<p class="muted state">
		{#if statement.closed}
			Closed{params.closed_at ? ` ${params.closed_at.slice(0, 10)}` : ''}.
		{:else}
			Open.
		{/if}
		{#if params.params_from && params.params_from !== month}
			Parameters carried forward from {monthName(params.params_from)}.
		{:else if !params.params_from}
			No parameters set.
		{/if}
		{#each params.stipends as s (s.party_id)}
			<span class="param">Stipend {name(s.party_id)}: <Amount value={s.amount} /></span>
		{/each}
		{#each params.funders as f (f.business_id)}
			<span class="param">{name(f.business_id)} funded by {name(f.funder_id)}</span>
		{/each}
	</p>

	{#each statement.edges as e (e.child_id + e.parent_id)}
		<section class="card">
			<h2>
				{name(e.child_id)} → {name(e.parent_id)}
				<StatusMark status={status(e, statement.closed)} />
			</h2>
			<dl class="summary">
				{#each summary as [key, label] (key)}
					<div>
						<dt>{label}</dt>
						<dd><Amount value={e[key]} /></dd>
					</div>
				{/each}
			</dl>

			{#if e.lines?.length}
				<table>
					<thead>
						<tr>
							<th>Date</th><th>Kind</th><th>Description</th><th>Route</th><th class="num">Amount</th
							>
						</tr>
					</thead>
					<tbody>
						{#each e.lines as l (`${l.transaction_id}/${l.position}/${l.kind}`)}
							{@const s = split(txs, l)}
							<tr>
								<td class="date">{l.date}</td>
								<td class="muted">{lineKinds[l.kind]}</td>
								<td>
									{s.tx?.description ?? '?'}
									{#if s.split?.description}<div class="muted note">{s.split.description}</div>{/if}
								</td>
								<td class="muted">
									{#if s.split}{legName(s.split.from, ref)} → {legName(s.split.to, ref)}{/if}
								</td>
								<td class="num"><Amount value={l.amount} /></td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}

			{#if e.in_transit?.length}
				<h3>In transit</h3>
				<table>
					<tbody>
						{#each e.in_transit as t (`${t.transaction_id}/${t.position}`)}
							<tr>
								<td class="date">{t.date}</td>
								<td>{split(txs, t).tx?.description ?? '?'}</td>
								<td class="muted">{t.cleared_by ? 'cleared' : 'not cleared yet'}</td>
								<td class="num"><Amount value={t.amount} /></td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</section>
	{/each}

	{#if statement.unpriced.length}
		<p class="muted">
			{statement.unpriced.length} splits have no value in the household currency and are left out.
		</p>
	{/if}
{:catch error}
	<Failure {error} />
{/await}

<style>
	.heading {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-5);
	}
	.months {
		display: flex;
		gap: var(--space-4);
		font-size: var(--text-sm);
	}
	.state {
		font-size: var(--text-sm);
	}
	.param {
		margin-left: var(--space-3);
	}
	.card {
		margin-bottom: var(--space-5);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.card h2 {
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
	}
	.summary {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
		gap: var(--space-3);
		margin: 0 0 var(--space-4);
	}
	.summary dt {
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
	.summary dd {
		margin: 0;
		font-size: var(--text-lg);
	}
	.date {
		white-space: nowrap;
		font-variant-numeric: var(--numeric);
	}
	.note {
		font-size: var(--text-xs);
	}
</style>
