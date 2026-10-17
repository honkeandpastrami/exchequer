<script lang="ts">
	import { resolve } from '$app/paths';
	import { all, api, ok } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import { today } from '#lib/dates.ts';
	import { accountKinds, holdings, partyKinds } from '#lib/labels.ts';
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
					<tr><th>Account</th><th>Kind</th><th class="num">Balance</th><th></th></tr>
				</thead>
				{#each g.sections as sec (sec.holding)}
					<tbody>
						{#each sec.rows as r (r.account.id)}
							<tr class:archived={r.account.archived}>
								<td>
									<a href={resolve(`/transactions?account_id=${r.account.id}`)}>{r.account.name}</a>
									{#if r.account.archived}<span class="muted tag">archived</span>{/if}
								</td>
								<td class="muted">{accountKinds[r.account.kind]}</td>
								<td class="num">
									{#each r.lines as l (l.commodity.id)}
										<div><Amount value={l.balance} code={l.commodity.code} /></div>
									{:else}
										<span class="muted">—</span>
									{/each}
								</td>
								<td class="check">
									<a href={resolve(`/checks?account=${r.account.id}`)}>Check</a>
								</td>
							</tr>
						{/each}
						<tr class="total">
							<td colspan="2">{holdings[sec.holding]} total</td>
							<td class="num">
								{#each sec.totals as l (l.commodity.id)}
									<div><Amount value={l.balance} code={l.commodity.code} /></div>
								{/each}
							</td>
							<td></td>
						</tr>
					</tbody>
				{/each}
				{#if g.net}
					<tfoot>
						<tr>
							<td colspan="2">Net</td>
							<td class="num">
								{#each g.net as l (l.commodity.id)}
									<div><Amount value={l.balance} code={l.commodity.code} /></div>
								{/each}
							</td>
							<td></td>
						</tr>
					</tfoot>
				{/if}
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
	tr.total td,
	tfoot td {
		font-weight: var(--weight-strong);
		border-top: var(--border-width) solid var(--color-border);
	}
	/* A gap between one holding's total and the next holding's accounts. */
	tbody + tbody tr:first-child td {
		padding-top: var(--space-4);
	}
	tfoot td {
		border-top-width: calc(var(--border-width) * 2);
	}
	.check {
		width: 1%;
		font-size: var(--text-sm);
	}
</style>
