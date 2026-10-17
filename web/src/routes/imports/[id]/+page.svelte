<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import { all, api, ApiError, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import NoValue from '#lib/components/NoValue.svelte';
	import SuggestionBadge from '#lib/components/SuggestionBadge.svelte';
	import { localDate } from '#lib/dates.ts';
	import { partyKinds, rank } from '#lib/labels.ts';
	import { hashQuery } from '#lib/query.ts';
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
	// The rows the filter picked, kept while you work on them (docs/ux.md §2):
	// a row you label stays on screen until the filter is picked again.
	let pinned: Set<string> | null = $state(null);
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
			if (shown) pin(shown);
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

	// `?row=0-3`: the row to show, its Details open (a transfer match's
	// other side links here).
	const focus = $derived(hashQuery(page.url).get('row'));
	const anchor = (r: { statement: number; position: number }) => `${r.statement}-${r.position}`;
	/** A link to a transfer match's other side. */
	const sideLink = (m: Schemas['TransferMatch']) =>
		`${resolve('/imports/[id]', { id: m.import_id })}?row=${anchor(m)}`;
	// Rows just brought into view, highlighted for a moment.
	let flashed: Set<string> = $state(new Set());
	let unflash: ReturnType<typeof setTimeout> | undefined;
	async function bring(targets: Row[]) {
		if (!targets.length) return;
		await tick();
		document.getElementById(`row-${anchor(targets[0])}`)?.scrollIntoView({ block: 'center' });
		flashed = new Set(targets.map(rowKey));
		clearTimeout(unflash);
		unflash = setTimeout(() => (flashed = new Set()), 2000);
	}
	async function reveal() {
		const row = focus ? rows.find((r) => anchor(r) === focus) : undefined;
		if (!row) return;
		show('');
		open = rowKey(row);
		await bring([row]);
	}
	function showUnlabeled() {
		show('unlabeled');
		bring(matching('unlabeled'));
	}
	// Another import is another page's worth of state.
	$effect(() => {
		void id;
		untrack(() => {
			imp = null;
			rows = [];
			open = null;
			result = null;
			notice = '';
			show('');
			load().then(reveal);
		});
	});
	$effect(() => {
		void focus;
		untrack(reveal);
	});

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

	const matching = (s: Status | 'unlabeled') =>
		s === 'unlabeled' ? unlabeled(rows) : rows.filter((r) => r.status === s);
	function pin(s: Status | 'unlabeled' | '') {
		pinned = s ? new Set(matching(s).map(rowKey)) : null;
	}
	function show(s: Status | 'unlabeled' | '') {
		shown = s;
		pin(s);
	}

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

	/** A select's option for a value: an unset purpose is the owner, no category NONE. */
	function optionOf(r: Reference, row: Row, label: Label, v: string | null): string {
		if (v !== null) return v;
		return label === 'purpose' ? (ownerOf(r, row) ?? '') : NONE;
	}

	/** What you'd type for a side: an account's label, a name, or "none". */
	function typed(r: Reference, side: Side): string {
		if (!side) return '';
		if ('account' in side) {
			const a = r.accounts.get(side.account);
			return a ? accountLabel(a, r) : '';
		}
		return side.counterparty ? (r.counterparties.get(side.counterparty)?.name ?? '') : 'none';
	}

	/**
	 * The suggestion a label's dropdown offers as an option of its own: an
	 * unsure one, or one you chose something else over (docs/ux.md §1, §5).
	 * None when it's what the field shows already.
	 */
	function offered(r: Reference, row: Row, label: Label): Schemas['LabelSuggestion'] | undefined {
		const st = labelState(row, label);
		if (st.kind === 'confident' || st.kind === 'missing') return undefined;
		const sg = row.suggestions.find((x) => x.label === label);
		if (!sg || st.kind === 'unsure') return sg;
		const same =
			label === 'counterparty'
				? typed(r, suggestedSide(sg)) === typed(r, sideOf(row))
				: optionOf(r, row, label, sg.value ?? null) ===
					optionOf(r, row, label, row[field[label]] ?? null);
		return same ? undefined : sg;
	}

	/** A suggestion's value, by name. */
	const suggested = (r: Reference, row: Row, label: Label, sg: Schemas['LabelSuggestion']) =>
		label === 'counterparty' ? typed(r, suggestedSide(sg)) : nameOf(r, row, label, sg.value);

	/**
	 * A typed other side: one of your accounts by its label (a transfer),
	 * else an existing counterparty by name, else a new one.
	 */
	async function counterparty(row: Row, name: string) {
		const r = ref;
		if (!r) return;
		const wanted = name.trim();
		// Cleared, or "none": deliberately no counterparty.
		if (!wanted || wanted.toLowerCase() === 'none') return setLabel(row, 'counterparty', null);
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

	// Suggestions, statuses and transfer matches worked out again; yours stay.
	let refreshing = $state(false);
	async function refresh() {
		refreshing = true;
		notice = '';
		await serial(async () => {
			ok(
				await api.POST('/api/v1/imports/{id}/refresh', {
					params: { path: { id }, header: { 'If-Match': etag } }
				})
			);
			await load();
		});
		refreshing = false;
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
	const visible = $derived(pinned ? rows.filter((r) => pinned?.has(rowKey(r))) : rows);
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
			there · {result.replaced} replaced · {result.skipped} not imported
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
			replacing, {planned.skip} not imported
		</button>
		<button class="quiet" disabled={busy || refreshing} onclick={refresh}
			>{refreshing ? 'Refreshing…' : 'Refresh suggestions'}</button
		>
		<button class="quiet" onclick={discard}>Discard</button>
		{#if missingAccount}<span class="muted small">Every statement needs an account first.</span
			>{:else if blocked.length}<span class="small warn"
				>{blocked.length} new {blocked.length === 1 ? 'row needs' : 'rows need'} labels:
				<button class="link" onclick={showUnlabeled}>show them</button></span
			>{/if}
	</div>

	<nav class="filter" aria-label="Rows to show">
		<button class="link" aria-pressed={shown === ''} onclick={() => show('')}
			>All {rows.length}</button
		>
		{#each allCounts as [status, n] (status)}
			<button class="link" aria-pressed={shown === status} onclick={() => show(status)}
				>{statuses[status]} {n}</button
			>
		{/each}
		<button class="link" aria-pressed={shown === 'unlabeled'} onclick={() => show('unlabeled')}
			>Needs labels {blocked.length}</button
		>
	</nav>

	<div class="list">
		<table>
			<colgroup>
				<col class="c-date" />
				<col />
				<col class="c-amount" />
				<col class="c-status" />
				<col class="c-decide" />
			</colgroup>
			<thead>
				<tr>
					<th>Date</th>
					<th>Description</th>
					<th class="num">Amount</th>
					<th>Status</th>
					<th>On commit</th>
				</tr>
			</thead>
			<tbody>
				{#each visible as row (rowKey(row))}
					{@const s = i.statements.find((x) => x.position === row.statement)}
					{@const code = s ? r.commodities.get(s.commodity_id)?.code : undefined}
					{@const tx = row.match_transaction_id ? txs.get(row.match_transaction_id) : undefined}
					{@const key = rowKey(row)}
					{@const labeled = row.decision === 'new' || row.decision === 'replace'}
					<tr
						id="row-{anchor(row)}"
						class:skip={row.decision === 'skip'}
						class:flash={flashed.has(key)}
					>
						<td class="date">{row.date}</td>
						<td>
							<div class="text">
								{row.set_description ?? row.description}
								{#if row.set_description}<span class="muted small">(bank: {row.description})</span
									>{/if}
							</div>
							{#if labeled}
								<!-- One slot per label, always in the same place. -->
								<div class="chips small">
									{#each labels as label (label)}
										{@const st = labelState(row, label)}
										{@const kept =
											row.decision === 'replace' && st.kind !== 'yours' && st.kind !== 'confident'}
										{@const v = valueOf(row, label)}
										<span class="chip">
											{#if !row.not_applicable.includes(label)}
												<span class="muted">{labelNames[label]}:</span>
												<span class="v">
													{#if kept}
														<span class="muted">unchanged</span>
													{:else if v === undefined}
														<NoValue kind="unknown" />
													{:else if label === 'counterparty'}
														{@const side = sideOf(row)}
														{#if side && 'counterparty' in side && side.counterparty === null}
															<NoValue kind="none" />
														{:else}
															{sideName(r, side)}
														{/if}
													{:else if label === 'category' && v === null}
														<NoValue kind="none" />
													{:else}
														{nameOf(r, row, label, v)}
													{/if}
												</span>
												{#if !kept && (st.kind === 'confident' || st.kind === 'unsure')}<SuggestionBadge
														suggestion={st.suggestion}
														compact
													/>{/if}
											{/if}
										</span>
									{/each}
								</div>
							{/if}
						</td>
						<td class="num"><Amount value={row.amount} {code} /></td>
						<td>
							{statuses[row.status]}
							<!-- The match on one line, cut short; Details shows it whole. -->
							{#if row.transfer_match}
								{@const m = row.transfer_match}
								{@const a = r.accounts.get(m.account_id)}
								<div class="small match">
									with
									<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- sideLink resolves -->
									<a href={sideLink(m)}>{a ? accountLabel(a, r) : '?'}: {m.date} {m.description}</a>
								</div>
							{:else if row.match_transaction_id}
								<div class="small match">
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
									aria-label="On commit"
								>
									{#each decisionsFor(row.status) as d (d)}
										<option value={d}>{decisionLabel(row.status, d)}</option>
									{/each}
								</select>
							{:else}
								<span class="muted">Pick the account</span>
							{/if}
							<button
								class="link"
								aria-expanded={open === key}
								onclick={() => (open = open === key ? null : key)}>Details</button
							>
						</td>
					</tr>
					{#if open === key}
						<tr class="details">
							<td colspan="5">
								{#if row.transfer_match}
									{@const m = row.transfer_match}
									{@const a = r.accounts.get(m.account_id)}
									<p class="matches">
										{statuses[row.status]} with
										<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- sideLink resolves -->
										<a href={sideLink(m)}
											>{a ? accountLabel(a, r) : '?'}: {m.date} {m.description}</a
										>
										{#if m.import_id !== i.id}(in {m.file_name ?? 'another import'}){/if}
									</p>
								{:else if row.match_transaction_id}
									<p class="matches">
										{statuses[row.status]} of
										<a href={resolve('/transactions/[id]', { id: row.match_transaction_id })}
											>{tx ? `${tx.date} ${tx.description}` : 'a transaction'}</a
										>
									</p>
								{/if}
								{#if labeled}
									<label class="field">
										<span>Description</span>
										<input
											value={row.set_description ?? ''}
											placeholder={row.description}
											onchange={(e) =>
												setRow(row, { set_description: e.currentTarget.value.trim() || null })}
										/>
									</label>
									<!-- Three fixed columns, each a control and one status line. -->
									<div class="fields">
										{#each labels as label (label)}
											{@const st = labelState(row, label)}
											{@const na = row.not_applicable.includes(label)}
											{@const sg = na ? undefined : offered(r, row, label)}
											{@const kept =
												row.decision === 'replace' &&
												st.kind !== 'yours' &&
												st.kind !== 'confident'}
											<div class="field">
												<label for="{key}-{label}">{labelNames[label]}</label>
												{#if na}
													<select id="{key}-{label}" disabled>
														<option>Not for a transfer</option>
													</select>
												{:else if label === 'counterparty'}
													{@const settled = st.kind === 'yours' || st.kind === 'confident'}
													<input
														id="{key}-{label}"
														list="{key}-sides"
														value={settled && typed(r, sideOf(row)) !== 'none'
															? typed(r, sideOf(row))
															: ''}
														placeholder={settled ? 'none' : 'A name, or one of your accounts'}
														onchange={(e) => counterparty(row, e.currentTarget.value)}
													/>
													<datalist id="{key}-sides">
														{#if sg}<option value={suggested(r, row, label, sg)}>Suggested</option
															>{/if}
														<option value="none">No counterparty</option>
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
														{#if sg}
															<option value={optionOf(r, row, label, sg.value ?? null)}
																>{suggested(r, row, label, sg)} (suggested)</option
															>
														{/if}
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
												<span class="status">
													{#if na}
														&nbsp;
													{:else if st.kind === 'yours'}
														Your choice{#if sg}; suggested: {suggested(r, row, label, sg)}{/if}
													{:else if st.kind === 'confident' || st.kind === 'unsure'}
														<SuggestionBadge suggestion={st.suggestion} />
														{#if kept}Keeps current; suggested: {suggested(
																r,
																row,
																label,
																st.suggestion
															)}?{:else}<span title={st.suggestion.reason}
																>{st.suggestion.reason}</span
															>{/if}
													{:else if kept}
														Keeps current
													{:else}
														<span class="warn">Pick one</span>
													{/if}
												</span>
											</div>
										{/each}
									</div>
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
	.actions {
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
	/* Fixed slots: a value changing never moves the others. */
	.chips {
		display: grid;
		grid-template-columns: minmax(0, 3fr) minmax(0, 2fr) minmax(0, 2fr);
		gap: var(--space-3);
		margin-top: var(--space-1);
	}
	/* A long value is cut short; its name and badge always show. */
	.chip {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-width: 0;
		height: calc(var(--text-sm) * var(--leading));
		overflow: hidden;
		white-space: nowrap;
	}
	.chip .v,
	.match,
	.status {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.matches {
		margin: 0 0 var(--space-3);
		overflow-wrap: anywhere;
	}
	.fields {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: var(--space-3);
		margin-top: var(--space-3);
	}
	@media (max-width: 40rem) {
		.chips,
		.fields {
			grid-template-columns: minmax(0, 1fr);
			gap: var(--space-1);
		}
	}
	.fields select,
	.fields input,
	.decide select,
	.details .field > input {
		width: 100%;
	}
	.status {
		min-height: calc(var(--text-xs) * var(--leading));
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
	.status > :global(* + *) {
		margin-left: var(--space-1);
	}
	.warn {
		color: var(--color-warning);
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
	/* Fixed columns: what's in a cell never resizes the others. */
	.list table {
		table-layout: fixed;
		min-width: 50rem;
	}
	.c-date {
		width: 6.5rem;
	}
	.c-amount {
		width: 8rem;
	}
	.c-status {
		width: 10rem;
	}
	/* Room for the longest decision, "Keep separate transactions". */
	.c-decide {
		width: 15.5rem;
	}
	.text {
		overflow-wrap: anywhere;
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
	.decide button {
		margin-top: var(--space-1);
	}
	.decide button[aria-expanded='true'] {
		font-weight: var(--weight-strong);
	}
	/* Just brought into view: a highlight that fades. */
	tr.flash td {
		animation: flash 2s ease-out;
	}
	@keyframes flash {
		from {
			background: var(--color-warning-bg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		tr.flash td {
			animation: none;
			background: var(--color-warning-bg);
		}
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
