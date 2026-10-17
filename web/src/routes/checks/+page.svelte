<script lang="ts">
	import { usd } from '#lib/usd.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { all, api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import { today } from '#lib/dates.ts';
	import { candidateReasons } from '#lib/labels.ts';
	import { negate } from '#lib/money.ts';
	import { hashQuery } from '#lib/query.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { reference, type Reference } from '#lib/reference.ts';
	import { transactionsById } from '#lib/transactions.ts';
	import { cleanQuantity } from '#lib/views/editor.ts';

	type Check = Schemas['BalanceCheck'];

	let account = $derived(hashQuery(page.url).get('account') ?? '');
	let ref: Reference | null = $state(null);
	let history: Check[] = $state([]);
	let shown: { check: Check; txs: Map<string, Schemas['Transaction']> } | null = $state(null);
	let form = $state({ amount: '', date: today(), commodity: '' });
	let failure: unknown = $state(null);
	let busy = $state(false);

	reference().then(
		(r) => {
			ref = r;
			form.commodity = usd(r)?.id ?? '';
		},
		(e) => (failure = e)
	);

	async function loadHistory(account: string) {
		shown = null;
		history = [];
		if (!account) return;
		try {
			history = await all((query) =>
				api.GET('/api/v1/balance-checks', { params: { query: { ...query, account_id: account } } })
			);
		} catch (e) {
			failure = e;
		}
	}

	$effect(() => {
		loadHistory(account);
	});

	async function show(id: string) {
		failure = null;
		try {
			const check = ok(await api.GET('/api/v1/balance-checks/{id}', { params: { path: { id } } }));
			const ids = (check.candidates ?? []).flatMap((c) => c.splits.map((s) => s.transaction_id));
			shown = { check, txs: await transactionsById(ids) };
		} catch (e) {
			failure = e;
		}
	}

	async function submit() {
		busy = true;
		failure = null;
		try {
			const check = ok(
				await api.POST('/api/v1/balance-checks', {
					body: {
						account_id: account,
						amount: cleanQuantity(form.amount),
						date: form.date || null,
						commodity_id: form.commodity || null
					}
				})
			);
			form.amount = '';
			await loadHistory(account);
			await show(check.id);
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function remove(c: Check) {
		if (!confirm(`Delete the check of ${c.date}?`)) return;
		failure = null;
		try {
			const r = await api.GET('/api/v1/balance-checks/{id}', { params: { path: { id: c.id } } });
			ok(r);
			done(
				await api.DELETE('/api/v1/balance-checks/{id}', {
					params: {
						path: { id: c.id },
						header: { 'If-Match': r.response.headers.get('etag') ?? '' }
					}
				})
			);
			await loadHistory(account);
		} catch (e) {
			failure = e;
		}
	}

	/** The split's effect on the checked account: + in, − out. */
	function movement(tx: Schemas['Transaction'] | undefined, position: number): string | null {
		const s = tx?.splits[position];
		if (!s) return null;
		return s.to.account_id === account ? s.to.quantity : negate(s.from.quantity);
	}

	const pick = (id: string) =>
		goto(resolve(id ? `/checks?account=${id}` : '/checks'), { replace: true });
</script>

<h1>Balance checks</h1>
<p class="muted intro">
	Enter what the bank shows. A match marks the account verified through that date; a mismatch lists
	transactions that could explain the difference, searched since the last match. For a credit card,
	enter what you owe as a positive amount.
</p>

{#if ref}
	{@const r = ref}
	{@const code = (id: string) => r.commodities.get(id)?.code}
	<form
		class="check"
		onsubmit={(e) => {
			e.preventDefault();
			submit();
		}}
	>
		<select value={account} onchange={(e) => pick(e.currentTarget.value)} aria-label="Account">
			<option value="">Account…</option>
			{#each accountsByOwner(r).filter((a) => !a.archived || a.id === account) as a (a.id)}
				<option value={a.id}>{accountLabel(a, r)}</option>
			{/each}
		</select>
		<input
			class="qty"
			inputmode="decimal"
			bind:value={form.amount}
			placeholder="Bank balance"
			aria-label="Bank balance"
		/>
		{#if r.commodities.size > 1}
			<select bind:value={form.commodity} aria-label="Commodity">
				{#each [...r.commodities.values()] as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
			</select>
		{/if}
		<input type="date" bind:value={form.date} aria-label="Date" />
		<button type="submit" disabled={busy || !account || !form.amount.trim()}>Check</button>
	</form>

	{#if failure}<Failure error={failure} />{/if}

	{#if shown}
		{@const c = shown.check}
		<section class="card result" class:ok={c.matches}>
			{#if c.matches}
				<h2>✓ Matches on {c.date}</h2>
				<p>
					The ledger shows <Amount value={c.ledger} code={code(c.commodity_id)} />. Verified through
					{c.verified_through}.
				</p>
			{:else}
				<h2>Off by <Amount value={c.difference} code={code(c.commodity_id)} /></h2>
				<p>
					On {c.date} the bank shows <Amount value={c.amount} /> and the ledger
					<Amount value={c.ledger} />.
					{#if c.searched_after}Searched the transactions after {c.searched_after}, the last date
						that matched.{:else}Searched every transaction: no earlier check matched.{/if}
				</p>
				{#each c.candidates ?? [] as cand, i (i)}
					<div class="candidate">
						<strong>{candidateReasons[cand.reason]}</strong>
						{#each cand.splits as sp (`${sp.transaction_id}/${sp.position}`)}
							{@const tx = shown.txs.get(sp.transaction_id)}
							{@const q = movement(tx, sp.position)}
							<div class="line">
								<span class="date">{tx?.date ?? ''}</span>
								<span class="desc">{tx?.description ?? '?'}</span>
								{#if q}<Amount value={q} />{/if}
								<a href={resolve('/transactions/[id]', { id: sp.transaction_id })}>Edit</a>
							</div>
						{/each}
					</div>
				{:else}
					<p class="muted">No transaction explains it on its own.</p>
				{/each}
			{/if}
		</section>
	{/if}

	{#if account}
		<table class="list">
			<thead>
				<tr>
					<th>Date</th>
					<th class="num">Bank</th>
					<th class="num">Ledger</th>
					<th class="num">Difference</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each history as c (c.id)}
					<tr class:selected={shown?.check.id === c.id}>
						<td>{c.date}</td>
						<td class="num"><Amount value={c.amount} code={code(c.commodity_id)} /></td>
						<td class="num"><Amount value={c.ledger} /></td>
						<td class="num">
							{#if c.matches}<span class="match">✓ matches</span>{:else}<Amount
									value={c.difference}
								/>{/if}
						</td>
						<td class="actions">
							<button class="link" onclick={() => show(c.id)}>Details</button>
							<button class="link danger" onclick={() => remove(c)}>Delete</button>
						</td>
					</tr>
				{:else}
					<tr><td colspan="5" class="muted">No checks yet.</td></tr>
				{/each}
			</tbody>
		</table>
	{/if}
{:else if failure}
	<Failure error={failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.intro,
	.check {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.qty {
		width: 10rem;
		text-align: right;
		font-variant-numeric: var(--numeric);
	}
	.card {
		margin-bottom: var(--space-4);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-left: 4px solid var(--color-warning);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.card.ok {
		border-left-color: var(--color-positive);
	}
	.candidate {
		margin-top: var(--space-3);
		font-size: var(--text-sm);
	}
	.line {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		padding: var(--space-1) 0 0 var(--space-4);
	}
	.date {
		font-variant-numeric: var(--numeric);
		color: var(--color-text-muted);
	}
	.desc {
		flex: 1;
	}
	.list {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.selected td {
		background: var(--color-surface-alt);
	}
	.match {
		color: var(--color-positive);
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.actions > * + * {
		margin-left: var(--space-3);
	}
</style>
