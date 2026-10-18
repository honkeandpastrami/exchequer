<script lang="ts">
	import { usd } from '#lib/usd.ts';
	import { resolve } from '$app/paths';
	import type { Schemas } from '#lib/api/client.ts';
	import { accountNumbers, accounts } from '#lib/api/resources.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import { accountKinds, partyKinds, rank } from '#lib/labels.ts';
	import type { Reference } from '#lib/reference.ts';
	import { referenceState } from '#lib/setup.svelte.ts';
	import { cleanQuantity } from '#lib/views/editor.ts';

	type Kind = Schemas['AccountKind'];
	/** An account's open form. */
	type Edit = {
		id: string;
		etag: string;
		name: string;
		kind: Kind;
		liquid: boolean;
		archived: boolean;
		limit: string;
		limitCommodity: string;
		numbers: Schemas['Number'][];
		newNumber: string;
	};

	const { state: s, reload } = referenceState();
	let showArchived = $state(false);
	let editing: Edit | null = $state(null);
	/** The form at the top: a new account. */
	let fresh: { name: string; owner: string; kind: Kind } | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);

	const kindList = Object.entries(accountKinds) as [Kind, string][];

	function groups(r: Reference) {
		return [...r.parties.values()]
			.sort(
				(a, b) =>
					rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)
			)
			.map((owner) => ({
				owner,
				accounts: [...r.accounts.values()]
					.filter((a) => a.owner_id === owner.id && (showArchived || !a.archived))
					.sort(
						(a, b) =>
							rank(accountKinds, a.kind) - rank(accountKinds, b.kind) ||
							a.name.localeCompare(b.name)
					)
			}))
			.filter((g) => g.accounts.length);
	}

	const household = (r: Reference) => usd(r)?.id ?? '';

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			editing = null;
			fresh = null;
			await reload();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function edit(a: Schemas['Account'], r: Reference) {
		if (editing?.id === a.id) {
			editing = null;
			return;
		}
		failure = null;
		try {
			const { item, etag } = await accounts.read(a.id);
			editing = {
				id: a.id,
				etag,
				name: item.name,
				kind: item.kind,
				liquid: item.liquid,
				archived: item.archived,
				limit: item.credit_limit?.quantity ?? '',
				limitCommodity: item.credit_limit?.commodity_id ?? household(r),
				numbers: item.numbers,
				newNumber: ''
			};
		} catch (e) {
			failure = e;
		}
	}

	const save = () =>
		run(async () => {
			if (!editing) return;
			const e = editing;
			// Only a credit card has a limit; another kind clears it.
			const limit = e.kind === 'credit_card' ? cleanQuantity(e.limit) : '';
			await accounts.update(e.id, e.etag, {
				name: e.name.trim(),
				kind: e.kind,
				liquid: e.liquid,
				archived: e.archived,
				credit_limit: limit ? { commodity_id: e.limitCommodity, quantity: limit } : null
			});
		});

	/** Number changes apply at once, not on Save. */
	async function changeNumbers(
		change: (id: string, etag: string) => Promise<{ item: Schemas['Account']; etag: string }>
	) {
		if (!editing) return;
		const e = editing;
		busy = true;
		failure = null;
		try {
			const { item, etag } = await change(e.id, e.etag);
			e.etag = etag;
			e.numbers = item.numbers;
			e.newNumber = '';
			// Saved already: the list shows it, whatever the form does next.
			await reload();
		} catch (err) {
			failure = err;
		} finally {
			busy = false;
		}
	}

	const addNumber = () =>
		changeNumbers((id, etag) => accountNumbers.add(id, etag, editing?.newNumber.trim() ?? ''));
	const removeNumber = (n: Schemas['Number']) =>
		changeNumbers((id, etag) => accountNumbers.remove(id, etag, n.id));

	async function remove(a: Schemas['Account']) {
		if (!editing) return;
		if (!confirm(`Delete the account “${a.name}”? Only an account nothing refers to can go.`))
			return;
		const etag = editing.etag;
		await run(() => accounts.remove(a.id, etag));
	}

	const add = () =>
		run(async () => {
			if (!fresh) return;
			await accounts.create({ name: fresh.name.trim(), owner_id: fresh.owner, kind: fresh.kind });
		});
</script>

<div class="heading">
	<h1>Accounts</h1>
	<button
		type="button"
		disabled={!s.ref || !!fresh}
		onclick={() => {
			failure = null;
			fresh = { name: '', owner: '', kind: 'checking' };
		}}>Add account</button
	>
</div>

{#if s.ref}
	{@const r = s.ref}
	{@const commodities = [...r.commodities.values()].sort((a, b) => a.code.localeCompare(b.code))}
	<p class="toolbar">
		<label class="toggle"><input type="checkbox" bind:checked={showArchived} /> Show archived</label
		>
	</p>

	{#if fresh}
		{@const f = fresh}
		<div class="adding">
			<RowForm label="Add account" {busy} {failure} onsave={add} oncancel={() => (fresh = null)}>
				<div class="fields">
					<label class="field grow">
						<span>Name</span>
						<!-- svelte-ignore a11y_autofocus -->
						<input bind:value={f.name} maxlength="100" required autofocus />
					</label>
					<label class="field">
						<span>Owner</span>
						<select bind:value={f.owner} required>
							<option value="" disabled>Choose…</option>
							{#each [...r.parties.values()] as p (p.id)}<option value={p.id}>{p.name}</option
								>{/each}
						</select>
					</label>
					<label class="field">
						<span>Kind</span>
						<select bind:value={f.kind}>
							{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
						</select>
					</label>
				</div>
			</RowForm>
		</div>
	{/if}

	{#each groups(r) as g (g.owner.id)}
		<section class="card">
			<h2>{g.owner.name}</h2>
			<table>
				<colgroup>
					<col class="c-name" />
					<col class="c-kind" />
					<col class="c-liquid" />
					<col class="c-limit" />
					<col class="c-numbers" />
					<col />
				</colgroup>
				<thead>
					<tr>
						<th>Name</th>
						<th>Kind</th>
						<th>Liquid</th>
						<th class="num">Credit limit</th>
						<th>Numbers</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each g.accounts as a (a.id)}
						<tr class:archived={a.archived}>
							<td>
								{a.name}
								{#if a.archived}<span class="badge">archived</span>{/if}
							</td>
							<td>{accountKinds[a.kind]}</td>
							<td>{a.liquid ? 'Yes' : 'No'}</td>
							<td class="num">
								{#if a.credit_limit}
									<Amount
										value={a.credit_limit.quantity}
										code={r.commodities.get(a.credit_limit.commodity_id)?.code}
									/>
								{/if}
							</td>
							<td>
								{#each a.numbers as n (n.id)}<span class="number">…{n.last4}</span>{/each}
							</td>
							<td class="actions">
								<a href={resolve(`/transactions?account_id=${a.id}`)}>Transactions</a>
								<button class="link" aria-expanded={editing?.id === a.id} onclick={() => edit(a, r)}
									>Edit</button
								>
							</td>
						</tr>
						{#if editing?.id === a.id}
							{@const e = editing}
							<tr class="open">
								<td colspan="6">
									<RowForm
										label="Edit {a.name}"
										{busy}
										{failure}
										onsave={save}
										oncancel={() => (editing = null)}
									>
										<div class="fields">
											<label class="field grow">
												<span>Name</span>
												<input bind:value={e.name} maxlength="100" required />
											</label>
											<label class="field">
												<span>Kind</span>
												<select bind:value={e.kind}>
													{#each kindList as [k, label] (k)}<option value={k}>{label}</option
														>{/each}
												</select>
											</label>
											<label class="check"
												><input type="checkbox" bind:checked={e.liquid} /> Liquid</label
											>
											{#if e.kind === 'credit_card'}
												<label class="field">
													<span>Credit limit</span>
													<span class="money">
														<input
															class="qty"
															inputmode="decimal"
															bind:value={e.limit}
															placeholder="None"
														/>
														<select bind:value={e.limitCommodity} aria-label="Limit commodity">
															{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option
																>{/each}
														</select>
													</span>
												</label>
											{/if}
											<label class="check"
												><input type="checkbox" bind:checked={e.archived} /> Archived</label
											>
										</div>
										<!-- Numbers change at once, not on Save. -->
										<div class="fields">
											<div class="field">
												<span>Numbers</span>
												<span class="numbers">
													{#each e.numbers as n (n.id)}
														<span class="number"
															>…{n.last4}<button
																type="button"
																class="link danger"
																onclick={() => removeNumber(n)}
																disabled={busy}
																aria-label={`Remove the number ending ${n.last4}`}>×</button
															></span
														>
													{:else}
														<span class="muted">none</span>
													{/each}
												</span>
											</div>
											<label class="field">
												<span>Add a number (it's saved at once)</span>
												<span class="money">
													<input
														bind:value={e.newNumber}
														autocomplete="off"
														maxlength="60"
														onkeydown={(ev) => {
															if (ev.key === 'Enter') {
																ev.preventDefault();
																if (e.newNumber.trim()) addNumber();
															}
														}}
													/>
													<button
														type="button"
														onclick={addNumber}
														disabled={busy || !e.newNumber.trim()}>Add</button
													>
												</span>
											</label>
										</div>
										{#snippet actions()}
											<button
												type="button"
												class="quiet danger"
												onclick={() => remove(a)}
												disabled={busy}>Delete</button
											>
										{/snippet}
									</RowForm>
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</section>
	{/each}
	{#if failure && !editing && !fresh}<Failure error={failure} />{/if}
{:else if s.failure}
	<Failure error={s.failure} />
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
	.toolbar {
		display: flex;
		justify-content: flex-end;
		margin: 0 0 var(--space-3);
	}
	.adding {
		margin-bottom: var(--space-4);
	}
	.open td {
		border-top: none;
		padding-top: 0;
	}
	.money,
	.numbers {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1);
	}
	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.card {
		margin-bottom: var(--space-4);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
		overflow-x: auto;
	}
	/* The same columns in every owner's card. */
	table {
		table-layout: fixed;
		min-width: 56rem;
	}
	.c-name {
		width: 20%;
	}
	.c-kind {
		width: 12%;
	}
	.c-liquid {
		width: 8%;
	}
	.c-limit {
		width: 18%;
	}
	.c-numbers {
		width: 17%;
	}
	.number {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		margin-right: var(--space-2);
		font-variant-numeric: tabular-nums;
	}
	.archived td {
		color: var(--color-text-muted);
	}
	.qty {
		width: 8rem;
		text-align: right;
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.actions > * + * {
		margin-left: var(--space-3);
	}
</style>
