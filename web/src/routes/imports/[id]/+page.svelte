<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { all, api, ApiError, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import SuggestionBadge from '#lib/components/SuggestionBadge.svelte';
	import { localDate } from '#lib/dates.ts';
	import { partyKinds, rank } from '#lib/labels.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { forgetReference, reference, sorted, type Reference } from '#lib/reference.ts';
	import { transactionsById } from '#lib/transactions.ts';
	import {
		counted,
		decisionLabel,
		decisionsFor,
		labelNames,
		labelState,
		noneNames,
		plan,
		rowKey,
		sideOf,
		statementName,
		statuses,
		suggestedSide,
		totals,
		unlabeled,
		type Side
	} from '#lib/views/imports.ts';

	type Row = Schemas['StagedRow'];
	type Status = Schemas['RowStatus'];
	type Label = Schemas['Label'];
	const labels = Object.keys(labelNames) as Label[];
	const field = {
		counterparty: 'counterparty_id',
		category: 'category_id',
		purpose: 'purpose_id'
	} as const;
	// The select value for an explicit "none".
	const NONE = '-';

	const id = $derived(page.params.id ?? '');

	let ref = $state<Reference | null>(null);
	let imp = $state<Schemas['Import'] | null>(null);
	let etag = '';
	let rows: Row[] = $state([]);
	let txs: Map<string, Schemas['Transaction']> = $state(new Map());
	// A status, or the rows that need labels.
	let shown = $state<Status | 'unlabeled' | ''>('');
	let open: string | null = $state(null);
	// Whether to remember each statement's number; yes unless unticked.
	let remember: Record<number, boolean> = $state({});
	let result: Schemas['Committed'] | null = $state(null);
	let notice = $state('');
	let failure: unknown = $state(null);
	let busy = $state(false);

	async function load() {
		try {
			const r = await api.GET('/api/v1/imports/{id}', { params: { path: { id } } });
			imp = ok(r);
			etag = r.response.headers.get('etag') ?? '';
			rows = await all((query) =>
				api.GET('/api/v1/imports/{id}/rows', { params: { path: { id }, query } })
			);
			const matched = rows.flatMap((r) => (r.match_transaction_id ? [r.match_transaction_id] : []));
			txs = await transactionsById(matched.filter((m) => !txs.has(m)));
		} catch (e) {
			failure = e;
		}
	}

	reference().then(
		(r) => (ref = r),
		(e) => (failure = e)
	);
	load();

	// Edits go one at a time: each needs the ETag the one before returned.
	let queue: Promise<unknown> = Promise.resolve();
	function serial(work: () => Promise<void>): Promise<void> {
		const next = queue.then(async () => {
			failure = null;
			try {
				await work();
			} catch (e) {
				failure = e;
				// Someone else changed the import: show it as it is now.
				if (e instanceof ApiError && e.status === 412) await load();
			}
		});
		queue = next;
		return next;
	}

	const setAccount = (s: Schemas['StagedStatement'], account: string) =>
		serial(async () => {
			const r = await api.PATCH('/api/v1/imports/{id}/statements/{position}', {
				params: { path: { id, position: s.position }, header: { 'If-Match': etag } },
				body: {
					account_id: account || null,
					remember_number: !!account && !!s.number_last4 && (remember[s.position] ?? true)
				}
			});
			ok(r);
			if (account && s.number_last4 && (remember[s.position] ?? true)) forgetReference();
			// The rows' statuses were worked out again.
			await load();
		});

	const setRow = (row: Row, body: Schemas['RowUpdate']) =>
		serial(async () => {
			const r = await api.PATCH('/api/v1/imports/{id}/rows/{statement}/{position}', {
				params: {
					path: { id, statement: row.statement, position: row.position },
					header: { 'If-Match': etag }
				},
				body
			});
			const updated = ok(r);
			etag = r.response.headers.get('etag') ?? '';
			rows = rows.map((x) => (rowKey(x) === rowKey(updated) ? updated : x));
		});

	const setLabel = (row: Row, label: Label, value: string | null) =>
		setRow(row, { [field[label]]: value });

	/** A label's value on screen: yours, else a suggestion's. */
	function valueOf(row: Row, label: Label): string | null | undefined {
		const st = labelState(row, label);
		if (st.kind === 'yours') return row[field[label]] ?? null;
		if (st.kind === 'missing') return undefined;
		return st.suggestion.value ?? null;
	}

	/** The other side's name: an account (a transfer), a counterparty, or none. */
	function sideName(r: Reference, side: Side): string {
		if (!side) return '?';
		if ('account' in side) {
			const a = r.accounts.get(side.account);
			return a ? `${accountLabel(a, r)} (transfer)` : '?';
		}
		return side.counterparty
			? (r.counterparties.get(side.counterparty)?.name ?? '?')
			: noneNames.counterparty;
	}

	/** Accounts a row's other side can be: not archived, not its own. */
	function transferAccounts(r: Reference, row: Row): Schemas['Account'][] {
		const own = imp?.statements.find((s) => s.position === row.statement)?.account_id;
		return accountsByOwner(r).filter((a) => !a.archived && a.id !== own);
	}

	/** The owner of a row's account: who an unset purpose means (design §4.1). */
	function ownerOf(r: Reference, row: Row): string | undefined {
		const own = imp?.statements.find((s) => s.position === row.statement)?.account_id;
		return own ? r.accounts.get(own)?.owner_id : undefined;
	}

	function nameOf(r: Reference, row: Row, label: Label, value: string | null | undefined): string {
		if (value === undefined) return '?';
		if (value === null && label === 'purpose') value = ownerOf(r, row) ?? null;
		if (value === null) return noneNames[label];
		const named =
			label === 'counterparty'
				? r.counterparties.get(value)
				: label === 'category'
					? r.categories.get(value)
					: r.parties.get(value);
		return named?.name ?? '?';
	}

	/**
	 * A select's value: an ID, NONE, or '' while missing. An unset purpose
	 * is the account's owner, picked by name like any other.
	 */
	function selected(r: Reference, row: Row, label: Label): string {
		const v = valueOf(row, label);
		if (v === undefined || labelState(row, label).kind === 'unsure') return '';
		if (v === null && label === 'purpose') return ownerOf(r, row) ?? '';
		return v ?? NONE;
	}

	/**
	 * A typed other side: one of your accounts by its label (a transfer),
	 * else an existing counterparty by name, else a new one.
	 */
	async function counterparty(row: Row, name: string) {
		const r = ref;
		if (!r) return;
		const wanted = name.trim();
		if (!wanted) return;
		const account = transferAccounts(r, row).find(
			(a) => accountLabel(a, r).toLowerCase() === wanted.toLowerCase()
		);
		if (account) return setRow(row, { transfer_account_id: account.id });
		let c = [...r.counterparties.values()].find(
			(c) => c.name.toLowerCase() === wanted.toLowerCase()
		);
		if (!c) {
			try {
				c = ok(await api.POST('/api/v1/counterparties', { body: { name: wanted } }));
				forgetReference();
				ref = await reference();
			} catch (e) {
				failure = e;
				return;
			}
		}
		return setRow(row, { counterparty_id: c.id });
	}

	async function commit() {
		busy = true;
		notice = '';
		await serial(async () => {
			const r = await api.POST('/api/v1/imports/{id}/commit', {
				params: { path: { id }, header: { 'If-Match': etag } }
			});
			if (r.response.status === 409 && r.error?.detail?.includes('again')) {
				notice = 'The ledger changed since these rows were checked. Here they are again.';
				await load();
				return;
			}
			result = ok(r);
			imp = null;
		});
		busy = false;
	}

	async function discard() {
		if (!confirm('Discard this import? Nothing of it enters the ledger.')) return;
		await serial(async () => {
			done(
				await api.DELETE('/api/v1/imports/{id}', {
					params: { path: { id }, header: { 'If-Match': etag } }
				})
			);
			await goto(resolve('/imports'));
		});
	}

	const missingAccount = $derived(imp?.statements.some((s) => !s.account_id) ?? false);
	const blocked = $derived(unlabeled(rows));
	const planned = $derived(plan(rows));
	const visible = $derived(
		shown === 'unlabeled' ? blocked : shown ? rows.filter((r) => r.status === shown) : rows
	);
	const allCounts = $derived(imp ? counted(totals(imp.statements)) : []);
	const owners = $derived(
		ref
			? [...ref.parties.values()].sort(
					(a, b) =>
						rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)
				)
			: []
	);
</script>

<p class="back"><a href={resolve('/imports')}>← Imports</a></p>

{#if result && ref}
	{@const r = ref}
	<h1>Committed</h1>
	<section class="card ok">
		<p>
			{result.created} new ({result.transfers} transfers with their match) · {result.matched} already
			there · {result.replaced} replaced · {result.skipped} skipped
		</p>
		{#if result.import_batch_id}
			<p>
				<a href={resolve(`/transactions?import_batch_id=${result.import_batch_id}`)}
					>See the new transactions</a
				>
			</p>
		{/if}
	</section>
	{#each result.balance_checks as b (b.id)}
		{@const a = r.accounts.get(b.account_id)}
		{@const code = r.commodities.get(b.commodity_id)?.code}
		<section class="card" class:ok={b.matches}>
			<strong>{a ? accountLabel(a, r) : '?'}</strong>:
			{#if b.matches}
				✓ the bank's <Amount value={b.amount} {code} /> on {b.date} matches the ledger.
			{:else}
				the bank says <Amount value={b.amount} {code} /> on {b.date}, the ledger
				<Amount value={b.ledger} {code} />: off by <Amount value={b.difference} {code} />.
				<a href={resolve(`/checks?account=${b.account_id}`)}>Look into it</a>
			{/if}
		</section>
	{/each}
{:else if imp && ref}
	{@const r = ref}
	{@const i = imp}
	<h1>{i.file_name ?? 'Import'}</h1>
	<p class="muted meta">
		{i.format.toUpperCase()} · uploaded {localDate(i.created_at)} · sha256 {i.file_sha256.slice(
			0,
			12
		)}
	</p>

	{#each i.statements as s (s.position)}
		{@const code = r.commodities.get(s.commodity_id)?.code}
		<section class="card statement" class:ok={!!s.account_id}>
			<div class="line">
				<strong>{statementName(s)}</strong>
				<select
					value={s.account_id ?? ''}
					onchange={(e) => setAccount(s, e.currentTarget.value)}
					aria-label="Account for {statementName(s)}"
				>
					<option value="">Which account?</option>
					{#each accountsByOwner(r).filter((a) => !a.archived || a.id === s.account_id) as a (a.id)}
						<option value={a.id}>{accountLabel(a, r)}</option>
					{/each}
				</select>
				{#if s.suggested_account && (s.account_suggested || !s.account_id)}
					{@const a = r.accounts.get(s.suggested_account.account_id)}
					<span class="suggested">
						{#if !s.account_suggested}Suggested: {a ? accountLabel(a, r) : '?'}{/if}
						<SuggestionBadge suggestion={s.suggested_account} />
						<button
							class="link"
							onclick={() => s.suggested_account && setAccount(s, s.suggested_account.account_id)}
							>{s.account_suggested ? 'Accept' : 'Use it'}</button
						>
					</span>
				{/if}
				{#if s.number_last4 && (!s.account_id || s.account_suggested)}
					<label class="check">
						<input
							type="checkbox"
							checked={remember[s.position] ?? true}
							onchange={(e) => (remember[s.position] = e.currentTarget.checked)}
						/>
						Remember …{s.number_last4} for this account
					</label>
				{/if}
			</div>
			<p class="muted small">
				{#each counted(s.counts) as [status, n], k (status)}{k ? ' · ' : ''}{n}
					{statuses[status].toLowerCase()}{:else}No rows{/each}
				{#if s.ledger}
					· bank balance <Amount value={s.ledger.amount} {code} /> on {s.ledger.date}{/if}
			</p>
			{#if s.skipped.length}
				<details class="small">
					<summary>{s.skipped.length} lines couldn't be read</summary>
					<ul>
						{#each s.skipped as l (l.line)}<li>Line {l.line}: {l.reason}</li>{/each}
					</ul>
				</details>
			{/if}
		</section>
	{:else}
		<p class="muted">The file has no statements.</p>
	{/each}

	{#if notice}<p class="notice" role="status">{notice}</p>{/if}
	{#if failure}<Failure error={failure} />{/if}

	<div class="actions">
		<button disabled={busy || missingAccount || blocked.length > 0} onclick={commit}>
			Commit: {planned.new} new, {planned.match} already there or transfer matches, {planned.replace}
			replacing, {planned.skip} skipped
		</button>
		<button class="quiet" onclick={discard}>Discard</button>
		{#if missingAccount}<span class="muted small">Every statement needs an account first.</span
			>{:else if blocked.length}<span class="small warn"
				>{blocked.length} new {blocked.length === 1 ? 'row needs' : 'rows need'} labels:
				<button class="link" onclick={() => (shown = 'unlabeled')}>show them</button></span
			>{/if}
	</div>

	<nav class="filter" aria-label="Rows to show">
		<button class="link" aria-pressed={shown === ''} onclick={() => (shown = '')}
			>All {rows.length}</button
		>
		{#each allCounts as [status, n] (status)}
			<button class="link" aria-pressed={shown === status} onclick={() => (shown = status)}
				>{statuses[status]} {n}</button
			>
		{/each}
		{#if blocked.length}
			<button
				class="link"
				aria-pressed={shown === 'unlabeled'}
				onclick={() => (shown = 'unlabeled')}>Needs labels {blocked.length}</button
			>
		{/if}
	</nav>

	<div class="list">
		<table>
			<thead>
				<tr>
					<th>Date</th>
					<th>Description</th>
					<th class="num">Amount</th>
					<th>Status</th>
					<th>Commit will</th>
				</tr>
			</thead>
			<tbody>
				{#each visible as row (rowKey(row))}
					{@const s = i.statements.find((x) => x.position === row.statement)}
					{@const code = s ? r.commodities.get(s.commodity_id)?.code : undefined}
					{@const tx = row.match_transaction_id ? txs.get(row.match_transaction_id) : undefined}
					{@const key = rowKey(row)}
					<tr class:skip={row.decision === 'skip'}>
						<td class="date">{row.date}</td>
						<td>
							{row.set_description ?? row.description}
							{#if row.set_description}<span class="muted small">(bank: {row.description})</span
								>{/if}
							{#if row.decision === 'new' || row.decision === 'replace'}
								<div class="chips small">
									{#each labels as label (label)}
										{@const st = labelState(row, label)}
										{@const kept =
											row.decision === 'replace' && st.kind !== 'yours' && st.kind !== 'confident'}
										{#if !row.not_applicable.includes(label)}
											<span
												class="chip"
												class:missing={!kept && (st.kind === 'missing' || st.kind === 'unsure')}
											>
												<span class="muted">{labelNames[label]}:</span>
												{kept
													? 'as it is'
													: label === 'counterparty'
														? sideName(r, sideOf(row))
														: nameOf(r, row, label, valueOf(row, label))}
												{#if st.kind === 'confident' || st.kind === 'unsure'}<SuggestionBadge
														suggestion={st.suggestion}
													/>{/if}
											</span>
										{/if}
									{/each}
								</div>
							{/if}
						</td>
						<td class="num"><Amount value={row.amount} {code} /></td>
						<td>
							{statuses[row.status]}
							{#if row.transfer_match}
								{@const m = row.transfer_match}
								{@const a = r.accounts.get(m.account_id)}
								<div class="small">
									with {a ? accountLabel(a, r) : '?'}: {m.date}
									{m.description}
									{#if m.import_id !== i.id}
										(in <a href={resolve('/imports/[id]', { id: m.import_id })}
											>{m.file_name ?? 'another import'}</a
										>){/if}
								</div>
							{/if}
							{#if row.match_transaction_id}
								<div class="small">
									of <a href={resolve('/transactions/[id]', { id: row.match_transaction_id })}
										>{tx ? `${tx.date} ${tx.description}` : 'a transaction'}</a
									>
								</div>
							{/if}
						</td>
						<td class="decide">
							{#if decisionsFor(row.status).length}
								<select
									value={row.decision}
									onchange={(e) =>
										setRow(row, { decision: e.currentTarget.value as Schemas['Decision'] })}
									aria-label="Decision"
								>
									{#each decisionsFor(row.status) as d (d)}
										<option value={d}>{decisionLabel(row.status, d)}</option>
									{/each}
								</select>
							{:else}
								<span class="muted">Pick the account</span>
							{/if}
							<button class="link" onclick={() => (open = open === key ? null : key)}
								>{open === key ? 'Close' : 'Details'}</button
							>
						</td>
					</tr>
					{#if open === key}
						<tr class="details">
							<td colspan="5">
								{#if row.decision === 'replace'}
									<p class="small muted">
										The transaction already there takes this row's date, amount and description, and
										the labels set here; labels left open stay as they are.
									</p>
								{/if}
								{#if row.decision === 'new' || row.decision === 'replace'}
									<div class="labels">
										<label class="field grow">
											<span>Description</span>
											<input
												value={row.set_description ?? ''}
												placeholder={row.description}
												onchange={(e) =>
													setRow(row, { set_description: e.currentTarget.value.trim() || null })}
											/>
										</label>
									</div>
									<div class="labels">
										{#each labels as label (label)}
											{@const st = labelState(row, label)}
											<div class="field">
												<label for="{key}-{label}">{labelNames[label]}</label>
												{#if row.not_applicable.includes(label)}
													<span class="muted">Not for a transfer</span>
												{:else if label === 'counterparty'}
													{@const side = sideOf(row)}
													{@const to =
														side && 'account' in side ? r.accounts.get(side.account) : undefined}
													<input
														id="{key}-{label}"
														list="{key}-sides"
														value={st.kind === 'yours' || st.kind === 'confident'
															? to
																? accountLabel(to, r)
																: row.counterparty_id
																	? (r.counterparties.get(row.counterparty_id)?.name ?? '')
																	: ''
															: ''}
														placeholder={st.kind === 'yours' || st.kind === 'confident'
															? noneNames.counterparty
															: 'A name, or one of your accounts'}
														onchange={(e) => counterparty(row, e.currentTarget.value)}
													/>
													<datalist id="{key}-sides">
														{#each transferAccounts(r, row) as a (a.id)}<option
																value={accountLabel(a, r)}>Transfer</option
															>{/each}
														{#each sorted(r.counterparties) as c (c.id)}<option value={c.name}
															></option>{/each}
													</datalist>
												{:else}
													<select
														id="{key}-{label}"
														value={selected(r, row, label)}
														onchange={(e) =>
															setLabel(
																row,
																label,
																e.currentTarget.value === NONE ? null : e.currentTarget.value
															)}
													>
														<option value="" disabled>Choose…</option>
														{#if label === 'category'}
															<option value={NONE}>{noneNames.category}</option>
															{#each sorted(r.categories) as c (c.id)}<option value={c.id}
																	>{c.name}</option
																>{/each}
														{:else}
															{#each owners as o (o.id)}<option value={o.id}>{o.name}</option
																>{/each}
														{/if}
													</select>
												{/if}
												<span class="hint">
													{#if st.kind === 'yours'}
														Yours.
														{#if row.suggestions.some((x) => x.label === label)}
															<button class="link" onclick={() => setRow(row, { revert: [label] })}
																>Back to the suggestion</button
															>
														{/if}
													{:else if st.kind === 'confident'}
														<SuggestionBadge suggestion={st.suggestion} />
														{st.suggestion.reason}
													{:else if st.kind === 'unsure'}
														{@const sg = st.suggestion}
														<SuggestionBadge suggestion={sg} />
														{label === 'counterparty'
															? sideName(r, suggestedSide(sg))
															: nameOf(r, row, label, sg.value)}? {sg.reason}
														<button
															class="link"
															onclick={() =>
																sg.transfer_account_id
																	? setRow(row, { transfer_account_id: sg.transfer_account_id })
																	: setLabel(row, label, sg.value ?? null)}>Accept</button
														>
													{:else if row.decision === 'replace'}
														Stays as it is.
													{:else}
														<span class="warn">Needed.</span>
													{/if}
													{#if label === 'counterparty' && !(st.kind === 'yours' && !row.counterparty_id && !row.transfer_account_id)}
														<button class="link" onclick={() => setLabel(row, label, null)}
															>{noneNames.counterparty}</button
														>
													{/if}
												</span>
											</div>
										{/each}
									</div>
								{:else if row.decision === 'match' && row.status === 'transfer_match'}
									<p class="small muted">
										This row and its match are the two sides of one transfer. Whichever file is
										committed first records it, with both rows; the other file's row is then already
										there.
									</p>
								{:else if row.decision === 'match' && row.status === 'transfer_duplicate'}
									<p class="small muted">
										This row is the other side of a transfer already in the ledger: nothing new is
										created, and the row is kept with that transfer.
									</p>
								{:else if row.decision === 'match'}
									<p class="small muted">
										Nothing new is created: this file's row is kept with the transaction it matches.
									</p>
								{/if}
								<dl class="raw">
									{#if row.native_id}<dt>Bank ID</dt>
										<dd>{row.native_id}</dd>{/if}
									{#each Object.entries(row.raw as Record<string, unknown>) as [k, v] (k)}
										<dt>{k}</dt>
										<dd>{typeof v === 'string' ? v : JSON.stringify(v)}</dd>
									{/each}
								</dl>
							</td>
						</tr>
					{/if}
				{:else}
					<tr><td colspan="5" class="muted">No rows.</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
{:else if failure}
	<Failure error={failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.back {
		margin: 0 0 var(--space-3);
		font-size: var(--text-sm);
	}
	.meta,
	.small {
		font-size: var(--text-sm);
	}
	.meta {
		font-family: var(--font-mono);
	}
	.card {
		margin-bottom: var(--space-3);
		padding: var(--space-3) var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-left: 4px solid var(--color-warning);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.card.ok {
		border-left-color: var(--color-positive);
	}
	.card p {
		margin: var(--space-2) 0 0;
	}
	.line,
	.actions,
	.labels {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
	}
	.suggested {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
		margin-top: var(--space-1);
	}
	.chip.missing {
		color: var(--color-warning);
	}
	.warn {
		color: var(--color-warning);
	}
	.hint {
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
	.hint > * + * {
		margin-left: var(--space-1);
	}
	.labels + .labels {
		margin-top: var(--space-3);
		align-items: flex-start;
	}
	.check {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}
	.actions {
		margin: var(--space-4) 0;
	}
	.notice {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-sm);
		background: var(--color-surface-alt);
	}
	.filter {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
	}
	.filter [aria-pressed='true'] {
		font-weight: var(--weight-strong);
		text-decoration: underline;
	}
	.list {
		overflow-x: auto;
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	td {
		vertical-align: top;
	}
	.date {
		white-space: nowrap;
		font-variant-numeric: var(--numeric);
	}
	.decide {
		white-space: nowrap;
	}
	.decide > * + * {
		margin-left: var(--space-2);
	}
	tr.skip td {
		color: var(--color-text-muted);
	}
	tr.details td {
		background: var(--color-surface-alt);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}
	.grow {
		flex: 1;
		min-width: 14rem;
	}
	.raw {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: var(--space-1) var(--space-3);
		margin: var(--space-3) 0 0;
		font-size: var(--text-xs);
		font-family: var(--font-mono);
	}
	.raw dt {
		color: var(--color-text-muted);
	}
	.raw dd {
		margin: 0;
		word-break: break-all;
	}
</style>
