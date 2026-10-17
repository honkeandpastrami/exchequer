<script lang="ts">
	import { usd } from '#lib/usd.ts';
	import { resolve } from '$app/paths';
	import type { Schemas } from '#lib/api/client.ts';
	import { accountNumbers, accounts } from '#lib/api/resources.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import { accountKinds, partyKinds, rank } from '#lib/labels.ts';
	import type { Reference } from '#lib/reference.ts';
	import { referenceState } from '#lib/setup.svelte.ts';
	import { cleanQuantity } from '#lib/views/editor.ts';

	type Kind = Schemas['AccountKind'];
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
	let fresh = $state({ name: '', owner: '', kind: 'checking' as Kind });
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
			await reload();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function edit(a: Schemas['Account'], r: Reference) {
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
			editing = null;
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

	const remove = (a: Schemas['Account']) =>
		run(async () => {
			const { etag } = await accounts.read(a.id);
			if (!confirm(`Delete the account “${a.name}”? Only an account nothing refers to can go.`))
				return;
			await accounts.remove(a.id, etag);
		});

	const add = () =>
		run(async () => {
			await accounts.create({ name: fresh.name.trim(), owner_id: fresh.owner, kind: fresh.kind });
			fresh.name = '';
		});
</script>

<h1>Accounts</h1>

{#if s.ref}
	{@const r = s.ref}
	{@const commodities = [...r.commodities.values()].sort((a, b) => a.code.localeCompare(b.code))}
	<form
		class="add"
		onsubmit={(e) => {
			e.preventDefault();
			add();
		}}
	>
		<input bind:value={fresh.name} placeholder="New account" maxlength="100" aria-label="Name" />
		<select bind:value={fresh.owner} aria-label="Owner">
			<option value="" disabled>Owner…</option>
			{#each [...r.parties.values()] as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
		</select>
		<select bind:value={fresh.kind} aria-label="Kind">
			{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
		</select>
		<button type="submit" disabled={busy || !fresh.name.trim() || !fresh.owner}>Add</button>
		<label class="toggle"><input type="checkbox" bind:checked={showArchived} /> Show archived</label
		>
	</form>

	{#if failure}<Failure error={failure} />{/if}

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
						{#if editing?.id === a.id}
							<tr class="editing">
								<td><input bind:value={editing.name} maxlength="100" aria-label="Name" /></td>
								<td>
									<select bind:value={editing.kind} aria-label="Kind">
										{#each kindList as [k, label] (k)}<option value={k}>{label}</option>{/each}
									</select>
								</td>
								<td>
									<input type="checkbox" bind:checked={editing.liquid} aria-label="Liquid" />
								</td>
								<td class="num">
									{#if editing.kind === 'credit_card'}
										<input
											class="qty"
											inputmode="decimal"
											bind:value={editing.limit}
											placeholder="None"
											aria-label="Credit limit"
										/>
										<select bind:value={editing.limitCommodity} aria-label="Limit commodity">
											{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
										</select>
									{/if}
								</td>
								<td>
									{#each editing.numbers as n (n.id)}
										<span class="number"
											>…{n.last4}<button
												class="link danger"
												onclick={() => removeNumber(n)}
												disabled={busy}
												aria-label={`Remove the number ending ${n.last4}`}>×</button
											></span
										>
									{/each}
									<form
										class="add-number"
										onsubmit={(e) => {
											e.preventDefault();
											addNumber();
										}}
									>
										<input
											bind:value={editing.newNumber}
											placeholder="Add number"
											autocomplete="off"
											maxlength="60"
											aria-label="Account number to add"
										/>
										<button type="submit" disabled={busy || !editing.newNumber.trim()}>Add</button>
									</form>
								</td>
								<td class="actions">
									<label class="toggle"
										><input type="checkbox" bind:checked={editing.archived} /> Archived</label
									>
									<button onclick={save} disabled={busy || !editing.name.trim()}>Save</button>
									<button class="quiet" onclick={() => (editing = null)}>Cancel</button>
								</td>
							</tr>
						{:else}
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
									<button class="link" onclick={() => edit(a, r)} disabled={busy}>Edit</button>
									<button class="link danger" onclick={() => remove(a)} disabled={busy}
										>Delete</button
									>
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</section>
	{/each}
	<p class="muted note">
		Liquid accounts hold spendable money; activity inside investment accounts is never shared. An
		account's owner can't change once it has been created. Numbers are the ones bank files name the
		account by (a card may have two: one for purchases, one for payments); only their last four are
		ever shown, and number changes are saved at once.
	</p>
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.add {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}
	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.add .toggle {
		margin-left: auto;
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
	.add-number {
		display: flex;
		gap: var(--space-1);
		margin-top: var(--space-1);
	}
	.add-number input {
		width: 7rem;
	}
	.archived td {
		color: var(--color-text-muted);
	}
	.editing td {
		background: var(--color-surface-alt);
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
	.note {
		font-size: var(--text-sm);
	}
</style>
