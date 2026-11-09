<script lang="ts">
	// A month's settlement (design §4.3): per edge of the settlement tree, its
	// figures and the transactions on its statement. Claims and Paid add up
	// transactions, so each links to them; the other figures are sums of
	// figures, or a setting.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import MonthControls from '#lib/components/MonthControls.svelte';
	import PageHeading from '#lib/components/PageHeading.svelte';
	import Section from '#lib/components/Section.svelte';
	import StatusMark from '#lib/components/StatusMark.svelte';
	import TransactionTable from '#lib/components/TransactionTable.svelte';
	import DateStepper from '#lib/components/dates/DateStepper.svelte';
	import { lastDay, thisMonth } from '#lib/dates.ts';
	import { transactionsPath, type Filter } from '#lib/filters.ts';
	import { fetched, latest } from '#lib/price-fetching.svelte.ts';
	import { lineKinds } from '#lib/labels.ts';
	import { reference } from '#lib/reference.ts';
	import { SETTLEMENT_CODE } from '#lib/settlement-currency.ts';
	import { status } from '#lib/views/settlement.ts';

	let month = $derived(page.params.month ?? thisMonth());

	async function load(month: string) {
		const [ref, statement] = await Promise.all([
			reference(),
			api
				.GET('/api/v1/settlements/{month}/statement', { params: { path: { month } } })
				.then(ok)
				.then(fetched)
		]);
		return { month, ref, statement };
	}

	// Bumped after closing, reopening or new parameters: the statement changes.
	let revision = $state(0);
	// Keeps the month's last statement on screen while it reloads.
	const data = latest(
		() => month,
		() => (void revision, load(month))
	);

	/** The figures, and the lines a figure adds up, if it does. */
	const summary = [
		['carried', 'Carried', null],
		['claims', 'Claims', 'claim'],
		['stipend', 'Stipend', null],
		['due', 'Due', null],
		['paid', 'Paid', 'payment'],
		['outstanding', 'Outstanding', null]
	] as const;

	const lines = (e: Schemas['Edge']): Filter => ({ settles: month, party_id: e.child_id });
</script>

<PageHeading>
	<h1>Settlement</h1>
	<a href={resolve(`/settlement?year=${month.slice(0, 4)}`)}>Year</a>
	<DateStepper
		value={month}
		granularity="month"
		max={thisMonth()}
		name="Month"
		onchange={(m) => goto(resolve('/settlement/[month]', { month: m }))}
	/>
</PageHeading>

{#await data.promise}
	{#if !data.value}<p class="muted">Loading…</p>{/if}
{:catch error}
	<Failure {error} />
{/await}

{#if data.value}
	{@const { ref, statement } = data.value}
	{@const name = (id: string) => ref.parties.get(id)?.name ?? '?'}
	{#key month}
		<MonthControls {month} {ref} onchange={() => revision++} />
	{/key}

	{#each statement.edges as e (e.child_id + e.parent_id)}
		{@const statementLines = e.lines ?? []}
		<section class="edge">
			<h2>
				{name(e.child_id)} → {name(e.parent_id)}
				<StatusMark status={status(e, statement.closed)} />
			</h2>
			<Section label="Figures">
				{#snippet children(look)}
					<dl class="summary" class:narrow={look === 'narrow'}>
						{#each summary as [key, label, line] (key)}
							<div>
								<dt>{label}</dt>
								<dd>
									{#if line}
										<a href={resolve(transactionsPath({ ...lines(e), line }))}
											><Amount value={e[key]} /></a
										>
									{:else}
										<Amount value={e[key]} />
									{/if}
								</dd>
							</div>
						{/each}
					</dl>
				{/snippet}
			</Section>
			{#key revision}
				<TransactionTable filter={lines(e)} label="On the statement" empty="Nothing this month.">
					{#snippet values(tx)}
						{#each statementLines.filter((l) => l.transaction_id === tx.id) as l (`${l.position}/${l.kind}`)}
							<dt>{lineKinds[l.kind]}</dt>
							<dd><Amount value={l.amount} code={SETTLEMENT_CODE} /></dd>
							<dd class="aside">
								{#if l.valuation}
									{@const leg = tx.splits[l.position]?.from.account_id
										? tx.splits[l.position]?.from
										: tx.splits[l.position]?.to}
									at {l.valuation.rate}
									{SETTLEMENT_CODE}/{ref.commodities.get(leg?.commodity_id ?? '')?.code ?? '?'}
									{#if l.valuation.stale}· stale price{/if}
								{/if}
							</dd>
						{/each}
					{/snippet}
				</TransactionTable>
			{/key}
		</section>
	{/each}

	{#if statement.unpriced.length}
		<p>
			<a href={resolve(transactionsPath({ unpriced: true, to: lastDay(month) }))}
				>{statement.unpriced.length} splits have no price in {SETTLEMENT_CODE} and are left out</a
			>
		</p>
	{/if}
{/if}

<style>
	.edge {
		margin-bottom: var(--space-6);
	}
	.edge h2 {
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
	}
	/* Wide, the figures are one row; narrow, one per line. */
	.summary {
		display: flex;
		gap: var(--space-5);
		margin: 0 0 var(--space-3);
	}
	.summary > div {
		flex: none;
	}
	.summary.narrow {
		display: grid;
		grid-template-columns: max-content max-content;
		gap: var(--space-1) var(--space-4);
	}
	.summary.narrow > div {
		display: contents;
	}
	.summary dt {
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
	.summary.narrow dt {
		font-size: inherit;
	}
	.summary dd {
		margin: 0;
		font-size: var(--text-lg);
	}
	.summary.narrow dd {
		font-size: inherit;
		text-align: right;
	}
</style>
