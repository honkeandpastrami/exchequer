<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import LegName from '#lib/components/LegName.svelte';
	import MonthControls from '#lib/components/MonthControls.svelte';
	import StatusMark from '#lib/components/StatusMark.svelte';
	import { addMonths, monthName, thisMonth } from '#lib/dates.ts';
	import { lineKinds } from '#lib/labels.ts';
	import { reference } from '#lib/reference.ts';
	import { transactionsById } from '#lib/transactions.ts';
	import { SETTLEMENT_CODE } from '#lib/usd.ts';
	import { status } from '#lib/views/settlement.ts';

	let month = $derived(page.params.month ?? thisMonth());

	async function load(month: string) {
		const [ref, statement] = await Promise.all([
			reference(),
			api.GET('/api/v1/settlements/{month}/statement', { params: { path: { month } } }).then(ok)
		]);
		const ids = statement.edges.flatMap((e) => [
			...(e.lines ?? []).map((l) => l.transaction_id),
			...(e.in_transit ?? []).map((t) => t.transaction_id)
		]);
		const txs = await transactionsById(ids);
		return { month, ref, statement, txs };
	}

	// Bumped after closing, reopening or new parameters: the statement changes.
	let revision = $state(0);
	let data = $derived((void revision, load(month)));
	// Keeps the month's last statement on screen while it reloads.
	let loaded = $state<Awaited<ReturnType<typeof load>> | null>(null);
	let shown = $derived(loaded?.month === month ? loaded : null);
	$effect(() => {
		const d = data;
		d.then((v) => {
			if (d === data) loaded = v;
		}).catch(() => {});
	});

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
	{#if !shown}<p class="muted">Loading…</p>{/if}
{:catch error}
	<Failure {error} />
{/await}

{#if shown}
	{@const { ref, statement, txs } = shown}
	{@const name = (id: string) => ref.parties.get(id)?.name ?? '?'}
	{#key month}
		<MonthControls {month} {ref} onchange={() => revision++} />
	{/key}

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
									{#if s.split}<LegName leg={s.split.from} {ref} /> → <LegName
											leg={s.split.to}
											{ref}
										/>{/if}
								</td>
								<td class="num">
									<Amount value={l.amount} />
									{#if l.valuation && s.split}
										{@const leg = s.split.from.account_id ? s.split.from : s.split.to}
										<div class="muted note">
											at {l.valuation.rate}
											{SETTLEMENT_CODE}/{ref.commodities.get(leg.commodity_id)?.code ?? '?'}
											{#if l.valuation.stale}· stale price{/if}
										</div>
									{/if}
								</td>
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
			{statement.unpriced.length} splits have no price in {SETTLEMENT_CODE} and are left out.
		</p>
	{/if}
{/if}

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
	.card {
		margin-bottom: var(--space-5);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
		overflow-x: auto;
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
