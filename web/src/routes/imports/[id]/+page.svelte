<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import { all, api, ApiError, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import Mark from '#lib/components/Mark.svelte';
	import NoValue from '#lib/components/NoValue.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import SplitLines, { type Field, type Offer } from '#lib/components/SplitLines.svelte';
	import SuggestionBadge from '#lib/components/SuggestionBadge.svelte';
	import { localDate } from '#lib/dates.ts';
	import { hashQuery } from '#lib/query.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { forgetReference, reference, type Reference } from '#lib/reference.ts';
	import { transactionsById } from '#lib/transactions.ts';
	import { newCounterparties, OUTSIDE } from '#lib/views/editor.ts';
	import {
		fixedSide,
		rowDraft,
		rowProblems,
		rowUpdate,
		whole,
		type RowDraft
	} from '#lib/views/review.ts';
	import {
		counted,
		decisionLabel,
		decisionsFor,
		labelNames,
		labelState,
		noneNames,
		plan,
		readinessNames,
		readinessTones,
		readinessWhy,
		rowKey,
		sideOf,
		statementName,
		statuses,
		suggestedSide,
		totals,
		undecided,
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
	// A status, or the rows that need labels or a decision.
	type Shown = Status | 'unlabeled' | 'undecided' | '';
	let shown = $state<Shown>('');
	// The rows the filter picked, kept while you work on them (docs/ux.md §2):
	// a row you label stays on screen until the filter is picked again.
	let pinned: Set<string> | null = $state(null);
	let open: string | null = $state(null);
	/** The open row's form: as it opened, and as it is now. */
	type Form = {
		before: RowDraft;
		draft: RowDraft;
		tried: boolean;
		busy: boolean;
		failure: unknown;
	};
	let form: Form | null = $state(null);
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
		openRow(row);
		await bring([row]);
	}
	function showOnly(s: 'unlabeled' | 'undecided') {
		show(s);
		bring(matching(s));
	}
	// Another import is another page's worth of state.
	$effect(() => {
		void id;
		untrack(() => {
			imp = null;
			rows = [];
			open = null;
			form = null;
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
	function serial(
		work: () => Promise<void>,
		fail: (e: unknown) => void = (e) => (failure = e)
	): Promise<void> {
		const next = queue.then(async () => {
			failure = null;
			try {
				await work();
			} catch (e) {
				fail(e);
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

	const setRow = (row: Row, body: Schemas['RowUpdate']) => serial(() => patch(row, body));

	async function patch(row: Row, body: Schemas['RowUpdate']) {
		{
			const r = await api.PATCH('/api/v1/imports/{id}/rows/{statement}/{position}', {
				params: {
					path: { id, statement: row.statement, position: row.position },
					header: { 'If-Match': etag }
				},
				body
			});
			const updated = ok(r);
			rows = rows.map((x) => (rowKey(x) === rowKey(updated) ? updated : x));
			// A transfer match's other side shares the decision, and the
			// import's readiness follows: both come from the server.
			if ('decision' in body && updated.transfer_match) await load();
			else {
				const i = await api.GET('/api/v1/imports/{id}', { params: { path: { id } } });
				imp = ok(i);
				etag = i.response.headers.get('etag') ?? '';
			}
		}
	}

	/** Where a row is: its statement's account and commodity. */
	function bankOf(row: Row) {
		const s = imp?.statements.find((x) => x.position === row.statement);
		return s?.account_id ? { account: s.account_id, commodity: s.commodity_id } : undefined;
	}

	/** Details: the row's form opens under it; again, it closes. */
	function openRow(row: Row) {
		const bank = bankOf(row);
		open = rowKey(row);
		const before = bank && ref ? rowDraft(row, bank, ref) : null;
		form = before
			? { before, draft: structuredClone(before), tried: false, busy: false, failure: null }
			: null;
	}
	function toggle(row: Row) {
		if (open === rowKey(row)) {
			open = null;
			form = null;
		} else openRow(row);
	}

	async function save(row: Row) {
		const f = form;
		if (!f || !ref) return;
		f.tried = true;
		if (rowProblems(row, f.draft, ref).length) return;
		f.busy = true;
		f.failure = null;
		await serial(
			async () => {
				let r = ref!;
				const fresh = newCounterparties(f.draft, r);
				for (const name of fresh) ok(await api.POST('/api/v1/counterparties', { body: { name } }));
				if (fresh.length) {
					forgetReference();
					r = ref = await reference();
				}
				const body = rowUpdate(row, $state.snapshot(f.before), $state.snapshot(f.draft), r);
				if (Object.keys(body).length) await patch(row, body);
				open = null;
				form = null;
			},
			(e) => (f.failure = e)
		);
		f.busy = false;
	}

	/** The other side of a form's one split: the leg that isn't the bank's. */
	const otherLeg = (row: Row, f: Form) =>
		fixedSide(row) === 'from' ? f.draft.splits[0].to : f.draft.splits[0].from;

	/** What a field's dropdown offers: the row's suggestion, while it's one split. */
	function offers(r: Reference, row: Row, f: Form, i: number): Partial<Record<Field, Offer>> {
		if (i > 0 || f.draft.splits.length > 1 || row.splits.length) return {};
		const out: Partial<Record<Field, Offer>> = {};
		const cp = offered(r, row, 'counterparty');
		if (cp && !row.not_applicable.includes('counterparty')) {
			const side = suggestedSide(cp);
			out.other = {
				label: suggested(r, row, 'counterparty', cp),
				apply: () => {
					const l = otherLeg(row, f);
					if (side && 'account' in side) Object.assign(l, { account: side.account, outside: '' });
					else
						Object.assign(l, {
							account: OUTSIDE,
							outside: side?.counterparty
								? (r.counterparties.get(side.counterparty)?.name ?? '')
								: ''
						});
				}
			};
		}
		for (const label of ['category', 'purpose'] as const) {
			const sg = row.not_applicable.includes(label) ? undefined : offered(r, row, label);
			if (sg)
				out[label] = {
					label: suggested(r, row, label, sg),
					apply: () => (f.draft.splits[0][label] = sg.value ?? '')
				};
		}
		return out;
	}

	const matching = (s: Exclude<Shown, ''>) =>
		s === 'unlabeled'
			? unlabeled(rows)
			: s === 'undecided'
				? undecided(rows)
				: rows.filter((r) => r.status === s);
	function pin(s: Shown) {
		pinned = s ? new Set(matching(s).map(rowKey)) : null;
	}
	function show(s: Shown) {
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
		return side.counterparty ? (r.counterparties.get(side.counterparty)?.name ?? '') : '';
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
		label === 'counterparty'
			? typed(r, suggestedSide(sg)) || noneNames.counterparty
			: nameOf(r, row, label, sg.value);

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

	const blocked = $derived(unlabeled(rows));
	const toDecide = $derived(undecided(rows));
	const planned = $derived(plan(rows));
	const visible = $derived(pinned ? rows.filter((r) => pinned?.has(rowKey(r))) : rows);
	const allCounts = $derived(imp ? counted(totals(imp.statements)) : []);
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
				<Mark tone="ok" label="Matches" /> the bank's <Amount value={b.amount} {code} /> on {b.date}
				is the ledger's.
			{:else}
				<Mark tone="warning" label="Doesn't match" /> the bank says <Amount
					value={b.amount}
					{code}
				/> on {b.date}, the ledger
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
		<button disabled={busy || i.readiness.state === 'needs_work'} onclick={commit}>
			Commit: {planned.new} new, {planned.match} already there or transfer matches, {planned.replace}
			replacing, {planned.skip} not imported
		</button>
		<button class="quiet" disabled={busy || refreshing} onclick={refresh}
			>{refreshing ? 'Refreshing…' : 'Refresh suggestions'}</button
		>
		<button class="quiet" onclick={discard}>Discard</button>
	</div>
	<p class="readiness">
		<Mark tone={readinessTones[i.readiness.state]} label={readinessNames[i.readiness.state]} />
		{#each readinessWhy(i.readiness) as why (why)}<span class="small why">· {why}</span>{/each}
		{#if blocked.length}<button class="link small" onclick={() => showOnly('unlabeled')}
				>show the rows that need labels</button
			>{/if}
		{#if toDecide.length}<button class="link small" onclick={() => showOnly('undecided')}
				>show the rows that need a decision</button
			>{/if}
	</p>

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
		<button class="link" aria-pressed={shown === 'undecided'} onclick={() => show('undecided')}
			>Needs a decision {toDecide.length}</button
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
						class="row"
						class:skip={row.decision === 'skip'}
						class:flash={flashed.has(key)}
					>
						<td class="date">{row.date}</td>
						<td class="r-desc">
							<div class="text">
								{row.set_description ?? row.description}
								{#if row.set_description}<span class="muted small">(bank: {row.description})</span
									>{/if}
							</div>
							{#if labeled}
								<!-- One slot per label, always in the same place. -->
								<div class="chips small">
									{#if row.splits.length}
										<!-- Split: where each part goes, in order. -->
										<span class="chip whole">
											<span class="muted">Split {row.splits.length} ways:</span>
											<span class="v"
												>{row.splits
													.map((p) =>
														sideName(
															r,
															p.transfer_account_id
																? { account: p.transfer_account_id }
																: { counterparty: p.counterparty_id ?? null }
														)
													)
													.join(', ')}</span
											>
										</span>
									{/if}
									{#each row.splits.length ? [] : labels as label (label)}
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
						<td class="num r-amount"><Amount value={row.amount} {code} /></td>
						<td class="r-state">
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
								{@const sd = row.suggested_decision}
								{@const unsure = !row.decided && !!sd && !sd.confident}
								<select
									value={unsure ? '' : row.decision}
									onchange={(e) =>
										setRow(row, { decision: e.currentTarget.value as Schemas['Decision'] })}
									aria-label="On commit"
								>
									{#if unsure}<option value="" disabled>Choose…</option>{/if}
									{#each decisionsFor(row.status) as d (d)}
										<option value={d}
											>{decisionLabel(row.status, d)}{!row.decided && sd?.decision === d
												? ' (suggested)'
												: ''}</option
										>
									{/each}
								</select>
								{#if sd}
									<div class="status">
										{#if row.decided}
											Your choice{#if sd.decision !== row.decision}; suggested: {decisionLabel(
													row.status,
													sd.decision
												)}{/if}
										{:else}
											<SuggestionBadge suggestion={sd} />
											<span title={sd.reason}>{sd.reason}</span>
										{/if}
									</div>
								{/if}
							{:else}
								<span class="muted">Pick the account</span>
							{/if}
							<button class="link" aria-expanded={open === key} onclick={() => toggle(row)}
								>Details</button
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
								{#snippet facts()}
									{#if row.native_id}<span>Bank ID {row.native_id}</span>{/if}
									{#each Object.entries(row.raw as Record<string, unknown>) as [k, v] (k)}
										<span>{k} {typeof v === 'string' ? v : JSON.stringify(v)}</span>
									{/each}
								{/snippet}
								{#if labeled && form}
									{@const f = form}
									{@const bank = bankOf(row)}
									{#snippet status(i: number, field: Field)}
										{@const label = field === 'other' ? 'counterparty' : field}
										{#if i === 0 && f.draft.splits.length === 1 && !row.splits.length}
											{@const st = labelState(row, label)}
											{@const sg = offered(r, row, label)}
											{@const kept =
												row.decision === 'replace' &&
												st.kind !== 'yours' &&
												st.kind !== 'confident'}
											{#if st.kind === 'yours'}
												<span
													title={sg
														? `Your choice; suggested: ${suggested(r, row, label, sg)}`
														: undefined}
													>Your choice{#if sg}; suggested: {suggested(r, row, label, sg)}{/if}</span
												>
											{:else if st.kind === 'confident' || st.kind === 'unsure'}
												<SuggestionBadge suggestion={st.suggestion} />
												{#if kept}Keeps current; suggested: {suggested(
														r,
														row,
														label,
														st.suggestion
													)}?{:else}<span title={st.suggestion.reason}>{st.suggestion.reason}</span
													>{/if}
											{:else if kept}
												Keeps current
											{:else}
												<span class="warn">Pick one</span>
											{/if}
										{/if}
									{/snippet}
									<RowForm
										label="Edit {row.description}"
										busy={f.busy}
										issues={f.tried ? rowProblems(row, f.draft, r) : []}
										failure={f.failure}
										onsave={() => save(row)}
										oncancel={() => toggle(row)}
										onreload={load}
										{facts}
									>
										<label class="field description">
											<span>Description</span>
											<input
												bind:value={f.draft.description}
												placeholder={row.description}
												maxlength="500"
											/>
										</label>
										{#if bank}
											<SplitLines
												bind:splits={f.draft.splits}
												ref={r}
												commodity={bank.commodity}
												fixed={{ account: bank.account, side: fixedSide(row) }}
												expected={whole(row)}
												canAdd={row.decision === 'new' && row.status !== 'transfer_match'}
												more={false}
												offers={(i) => offers(r, row, f, i)}
												{status}
											/>
										{/if}
									</RowForm>
								{:else}
									<p class="facts muted">{@render facts()}</p>
								{/if}
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
	.chip.whole {
		grid-column: 1 / -1;
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
	.facts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
		font-size: var(--text-xs);
		font-family: var(--font-mono);
	}
	.description {
		margin-bottom: var(--space-3);
	}
	.description input {
		width: 100%;
	}
	.matches {
		margin: 0 0 var(--space-3);
		overflow-wrap: anywhere;
	}
	@media (max-width: 40rem) {
		.chips {
			grid-template-columns: minmax(0, 1fr);
			gap: var(--space-1);
		}
	}
	.decide select {
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
	.readiness {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2);
	}
	.why {
		color: var(--color-text-muted);
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
	/* A phone: each row a block (#21), the description across the full
	   width, then date and amount, status, and what commit does. */
	@media (max-width: 40rem) {
		.list table,
		.list tbody,
		.list tr,
		.list td {
			display: block;
		}
		.list table {
			min-width: 0;
		}
		.list thead,
		.list colgroup {
			display: none;
		}
		.list tr.row {
			display: grid;
			grid-template-columns: minmax(0, 1fr) auto;
			grid-template-areas:
				'desc desc'
				'date amount'
				'state state'
				'decide decide';
			gap: var(--space-1) var(--space-3);
			padding: var(--space-3);
			border-top: var(--border-width) solid var(--color-border);
		}
		.list tr.row td {
			padding: 0;
			border: none;
		}
		.r-desc {
			grid-area: desc;
		}
		.list .date {
			grid-area: date;
			color: var(--color-text-muted);
			font-size: var(--text-sm);
		}
		.r-amount {
			grid-area: amount;
		}
		.r-state {
			grid-area: state;
			font-size: var(--text-sm);
		}
		.decide {
			grid-area: decide;
		}
		.list tr.details td {
			padding: 0 var(--space-2) var(--space-3);
		}
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}
</style>
