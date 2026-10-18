<script lang="ts">
	import { usd } from '#lib/usd.ts';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { all, api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import Mark from '#lib/components/Mark.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
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
	/** The form at the top: a new check. */
	let form: { account: string; amount: string; date: string; commodity: string } | null =
		$state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);

	reference().then(
		(r) => (ref = r),
		(e) => (failure = e)
	);

	/** A check to open once its account's checks are loaded. */
	let opening: string | null = null;

	async function loadHistory(account: string) {
		shown = null;
		history = [];
		if (!account) return;
		try {
			history = await all((query) =>
				api.GET('/api/v1/balance-checks', { params: { query: { ...query, account_id: account } } })
			);
			const id = opening;
			opening = null;
			if (id && history.some((c) => c.id === id)) await show(id);
		} catch (e) {
			failure = e;
		}
	}

	$effect(() => {
		loadHistory(account);
	});

	async function show(id: string) {
		if (shown?.check.id === id) {
			shown = null;
			return;
		}
		failure = null;
		try {
			const check = ok(await api.GET('/api/v1/balance-checks/{id}', { params: { path: { id } } }));
			const ids = (check.candidates ?? []).flatMap((c) => c.splits.map((s) => s.transaction_id));
			shown = { check, txs: await transactionsById(ids) };
		} catch (e) {
			failure = e;
		}
	}

	function adding(r: Reference) {
		failure = null;
		form = { account, amount: '', date: today(), commodity: usd(r)?.id ?? '' };
	}

	async function submit() {
		const f = form;
		if (!f) return;
		busy = true;
		failure = null;
		try {
			const check = ok(
				await api.POST('/api/v1/balance-checks', {
					body: {
						account_id: f.account,
						amount: cleanQuantity(f.amount),
						date: f.date || null,
						commodity_id: f.commodity || null
					}
				})
			);
			form = null;
			// The account's checks, the new one open: another account's are
			// loaded when the page moves to it.
			opening = check.id;
			if (f.account !== account) await pick(f.account);
			else await loadHistory(account);
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

<div class="heading">
	<h1>Balance checks</h1>
	<button type="button" disabled={!ref || !!form} onclick={() => ref && adding(ref)}
		>Add balance check</button
	>
</div>
<p class="muted intro">
	Enter what the bank shows. A match marks the account verified through that date; a mismatch lists
	transactions that could explain the difference, searched since the last match. For a credit card,
	enter what you owe as a positive amount.
</p>

{#if ref}
	{@const r = ref}
	{@const code = (id: string) => r.commodities.get(id)?.code}
	{@const accountsShown = accountsByOwner(r).filter((a) => !a.archived || a.id === account)}
	<p class="toolbar">
		<select value={account} onchange={(e) => pick(e.currentTarget.value)} aria-label="Account">
			<option value="">Account…</option>
			{#each accountsShown as a (a.id)}
				<option value={a.id}>{accountLabel(a, r)}</option>
			{/each}
		</select>
	</p>

	{#if form}
		{@const f = form}
		<div class="adding">
			<RowForm
				label="Add balance check"
				{busy}
				{failure}
				onsave={submit}
				oncancel={() => (form = null)}
			>
				<div class="fields">
					<label class="field">
						<span>Account</span>
						<select bind:value={f.account} required>
							<option value="" disabled>Choose…</option>
							{#each accountsShown as a (a.id)}
								<option value={a.id}>{accountLabel(a, r)}</option>
							{/each}
						</select>
					</label>
					<label class="field">
						<span>Bank balance</span>
						<span class="money">
							<input class="qty" inputmode="decimal" bind:value={f.amount} required />
							{#if r.commodities.size > 1}
								<select bind:value={f.commodity} aria-label="Commodity">
									{#each [...r.commodities.values()] as c (c.id)}<option value={c.id}
											>{c.code}</option
										>{/each}
								</select>
							{/if}
						</span>
					</label>
					<label class="field">
						<span>On</span>
						<input type="date" bind:value={f.date} />
					</label>
				</div>
			</RowForm>
		</div>
	{/if}

	{#if failure && !form}<Failure error={failure} />{/if}

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
							{#if c.matches}<Mark tone="ok" label="Matches" />{:else}<Amount
									value={c.difference}
								/>{/if}
						</td>
						<td class="actions">
							<button
								class="link"
								aria-expanded={shown?.check.id === c.id}
								onclick={() => show(c.id)}>Details</button
							>
						</td>
					</tr>
					{#if shown && shown.check.id === c.id}
						{@const d = shown}
						<tr class="open">
							<td colspan="5">
								<section class="panel" class:ok={c.matches}>
									{#if c.matches}
										<h2>Matches on {c.date}</h2>
										<p>
											The ledger shows <Amount value={c.ledger} code={code(c.commodity_id)} />.
											Verified through {d.check.verified_through}.
										</p>
									{:else}
										<h2>Off by <Amount value={c.difference} code={code(c.commodity_id)} /></h2>
										<p>
											On {c.date} the bank shows <Amount value={c.amount} /> and the ledger
											<Amount value={c.ledger} />.
											{#if d.check.searched_after}Searched the transactions after {d.check
													.searched_after}, the last date that matched.{:else}Searched every
												transaction: no earlier check matched.{/if}
										</p>
										{#each d.check.candidates ?? [] as cand, i (i)}
											<div class="candidate">
												<strong>{candidateReasons[cand.reason]}</strong>
												{#each cand.splits as sp (`${sp.transaction_id}/${sp.position}`)}
													{@const tx = d.txs.get(sp.transaction_id)}
													{@const q = movement(tx, sp.position)}
													<div class="line">
														<span class="date">{tx?.date ?? ''}</span>
														<span class="desc">{tx?.description ?? '?'}</span>
														{#if q}<Amount value={q} />{/if}
														<a href={resolve('/transactions/[id]', { id: sp.transaction_id })}
															>Edit</a
														>
													</div>
												{/each}
											</div>
										{:else}
											<p class="muted">No transaction explains it on its own.</p>
										{/each}
									{/if}
									<p class="panel-actions">
										<button type="button" class="quiet danger" onclick={() => remove(c)}
											>Delete</button
										>
									</p>
								</section>
							</td>
						</tr>
					{/if}
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
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-4);
	}
	.intro,
	.toolbar,
	.adding {
		margin-bottom: var(--space-4);
	}
	.money {
		display: flex;
		gap: var(--space-1);
	}
	.qty {
		width: 10rem;
		text-align: right;
		font-variant-numeric: var(--numeric);
	}
	.open td {
		border-top: none;
		padding-top: 0;
	}
	/* A check's result, under its row. */
	.panel {
		padding: var(--space-3) var(--space-4);
		background: var(--color-surface-alt);
		border: var(--border-width) solid var(--color-border);
		border-left: 4px solid var(--color-warning);
		border-radius: var(--radius-md);
	}
	.panel.ok {
		border-left-color: var(--color-positive);
	}
	.panel h2 {
		margin: 0;
		font-size: var(--text-lg);
	}
	.panel-actions {
		display: flex;
		justify-content: flex-end;
		margin: var(--space-3) 0 0;
	}
	.danger {
		color: var(--color-negative);
		border-color: var(--color-negative);
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
	.actions {
		text-align: right;
		white-space: nowrap;
	}
</style>
