<script lang="ts">
	import { all, api, ok } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import { today } from '#lib/dates.ts';
	import { accountKinds, partyKinds } from '#lib/labels.ts';
	import { groupBalances } from '#lib/views/accounts.ts';

	let date = $state(today());
	let showArchived = $state(false);

	async function load(date: string, showArchived: boolean) {
		const [parties, commodities, accounts, balances] = await Promise.all([
			all((query) => api.GET('/api/v1/parties', { params: { query } })),
			all((query) => api.GET('/api/v1/commodities', { params: { query } })),
			all((query) => api.GET('/api/v1/accounts', { params: { query } })),
			api.GET('/api/v1/reports/balances', { params: { query: { date } } }).then(ok)
		]);
		return groupBalances(parties, commodities, accounts, balances.balances, showArchived);
	}

	let groups = $derived(load(date, showArchived));
</script>

<div class="heading">
	<h1>Accounts</h1>
	<label>Balances at the end of <input type="date" bind:value={date} required /></label>
	<label class="check"><input type="checkbox" bind:checked={showArchived} /> Show archived</label>
</div>

{#await groups}
	<p class="muted">Loading…</p>
{:then groups}
	{#each groups as g (g.owner.id)}
		<section class="owner">
			<h2>{g.owner.name} <span class="muted kind">{partyKinds[g.owner.kind]}</span></h2>
			<table>
				<thead>
					<tr><th>Account</th><th>Kind</th><th class="num">Balance</th></tr>
				</thead>
				<tbody>
					{#each g.rows as r (r.account.id)}
						<tr class:archived={r.account.archived}>
							<td>
								{r.account.name}
								{#if r.account.archived}<span class="muted tag">archived</span>{/if}
								{#if !r.account.liquid}<span class="muted tag">not liquid</span>{/if}
							</td>
							<td class="muted">{accountKinds[r.account.kind]}</td>
							<td class="num">
								{#each r.lines as l (l.commodity.id)}
									<div><Amount value={l.balance} code={l.commodity.code} /></div>
								{:else}
									<span class="muted">—</span>
								{/each}
							</td>
						</tr>
					{/each}
				</tbody>
				<tfoot>
					<tr>
						<td colspan="2">Total</td>
						<td class="num">
							{#each g.totals as l (l.commodity.id)}
								<div><Amount value={l.balance} code={l.commodity.code} /></div>
							{/each}
						</td>
					</tr>
				</tfoot>
			</table>
		</section>
	{:else}
		<p class="muted">No accounts yet.</p>
	{/each}
{:catch error}
	<Failure {error} />
{/await}

<style>
	.heading {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-5);
		margin-bottom: var(--space-4);
	}
	.heading h1 {
		margin: 0;
	}
	label {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.owner {
		margin-bottom: var(--space-5);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.kind,
	.tag {
		font-family: var(--font-ui);
		font-size: var(--text-xs);
		font-weight: var(--weight-normal);
	}
	.tag {
		margin-left: var(--space-2);
	}
	/* The same columns in every owner's table. */
	table {
		table-layout: fixed;
	}
	th:first-child {
		width: 50%;
	}
	tr.archived td {
		opacity: 0.7;
	}
	tfoot td {
		font-weight: var(--weight-strong);
		border-top: var(--border-width) solid var(--color-border);
	}
</style>
