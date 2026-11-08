<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import { all, api, ApiError, done, ok, type Schemas } from '#lib/api/client.ts';
	import Amount from '#lib/components/Amount.svelte';
	import CommittedImport from '#lib/components/CommittedImport.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import Field from '#lib/components/Field.svelte';
	import Mark from '#lib/components/Mark.svelte';
	import NoValue from '#lib/components/NoValue.svelte';
	import Quantities from '#lib/components/Quantities.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import RowStatus from '#lib/components/RowStatus.svelte';
	import Section from '#lib/components/Section.svelte';
	import SplitLines, { type Field as SplitField } from '#lib/components/SplitLines.svelte';
	import SuggestionBadge from '#lib/components/SuggestionBadge.svelte';
	import Tags from '#lib/components/Tags.svelte';
	import TransactionRow from '#lib/components/TransactionRow.svelte';
	import Values from '#lib/components/Values.svelte';
	import { dayName, stampName } from '#lib/dates.ts';
	import { negate } from '#lib/money.ts';
	import { hashQuery } from '#lib/query.ts';
	import { accountLabel, accountsByOwner } from '#lib/names.ts';
	import { forgetReference, reference, sorted, type Reference } from '#lib/reference.ts';
	import { transactionsById } from '#lib/transactions.ts';
	import { newCounterparties } from '#lib/views/editor.ts';
	import {
		fixedSide,
		rowDraft,
		rowProblems,
		rowUpdate,
		whole,
		type RowDraft
	} from '#lib/views/review.ts';
	import {
		choiceName,
		commitLine,
		counted,
		labelNames,
		labelState,
		rowKey,
		sideOf,
		stateOrder,
		states,
		statusWords,
		unreviewedQuestion,
		type Side
	} from '#lib/views/imports.ts';
	import { effectOn } from '#lib/views/transactions.ts';

	type Row = Schemas['StagedRow'];
	type State = Schemas['ReviewState'];
	type Label = Schemas['Label'];
	const labels = Object.keys(labelNames) as Label[];

	const id = $derived(page.params.id ?? '');

	let ref = $state<Reference | null>(null);
	let imp = $state<Schemas['Import'] | null>(null);
	let etag = '';
	let rows: Row[] = $state([]);
	let txs: Map<string, Schemas['Transaction']> = $state(new Map());
	// The rows of one state, or all.
	let shown = $state<State | ''>('');
	// The rows the filter picked, kept while you work on them (docs/ux.md §2):
	// a row you change stays on screen until the filter is picked again.
	let pinned: Set<string> | null = $state(null);
	let open: string | null = $state(null);
	/** The open row's form: as it is now. */
	type Form = {
		draft: RowDraft;
		tried: boolean;
		busy: boolean;
		failure: unknown;
	};
	let form: Form | null = $state(null);
	let result: Schemas['Committed'] | null = $state(null);
	/** The file's name, kept once it's committed. */
	let name = $state('');
	let notice = $state('');
	let failure: unknown = $state(null);
	let busy = $state(false);
	/** Commit asked about rows to review, and waits for an answer. */
	let asking = $state(false);

	async function load() {
		try {
			const r = await api.GET('/api/v1/imports/{id}', { params: { path: { id } } });
			imp = ok(r);
			name = imp.file_name ?? fileFormat(imp);
			etag = r.response.headers.get('etag') ?? '';
			rows = await all((query) =>
				api.GET('/api/v1/imports/{id}/rows', { params: { path: { id }, query } })
			);
			// The rows on screen stay there, changed or not (docs/ux.md §2).
			if (shown && !pinned) pin(shown);
			const matched = rows.flatMap((r) => (r.match_transaction_id ? [r.match_transaction_id] : []));
			const fresh = await transactionsById(matched.filter((m) => !txs.has(m)));
			if (fresh.size) txs = new Map([...txs, ...fresh]);
		} catch (e) {
			failure = e;
		}
	}

	const fileFormat = (i: Schemas['Import']) => `${i.format.toUpperCase()} file`;

	reference().then(
		(r) => (ref = r),
		(e) => (failure = e)
	);

	// `?row=0-3`: the row to show, its form open (a transfer pair's other
	// row links here).
	const focus = $derived(hashQuery(page.url).get('row'));
	const anchor = (r: { statement: number; position: number }) => `${r.statement}-${r.position}`;
	/** A link to a transfer pair's other row. */
	const sideLink = (m: Schemas['TransferPair']) =>
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
			asking = false;
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

	/** Picks a statement's account, or ticks its suggestion reviewed; the
	 * rows are worked out again. */
	const setStatement = (s: Schemas['StagedStatement'], body: Schemas['StatementUpdate']) =>
		serial(async () => {
			const r = await api.PATCH('/api/v1/imports/{id}/statements/{position}', {
				params: { path: { id, position: s.position }, header: { 'If-Match': etag } },
				body
			});
			ok(r);
			// The file's number may now be the account's.
			if (s.number_last4) forgetReference();
			await load();
		});

	const setRow = (row: Row, body: Schemas['RowUpdate']) =>
		serial(async () => {
			await patch(row, body);
		});

	/** Ticks these rows reviewed, or clears them; the server leaves out those
	 * that can't be. */
	const markReviewed = (targets: Row[], reviewed: boolean) =>
		serial(async () => {
			const keys = targets.map(({ statement, position }) => ({ statement, position }));
			for (let k = 0; k < keys.length; k += 500) {
				const r = await api.POST('/api/v1/imports/{id}/review', {
					params: { path: { id }, header: { 'If-Match': etag } },
					body: { rows: keys.slice(k, k + 500), reviewed }
				});
				imp = ok(r);
				etag = r.response.headers.get('etag') ?? '';
			}
			await load();
		});

	async function patch(row: Row, body: Schemas['RowUpdate']): Promise<Row> {
		const r = await api.PATCH('/api/v1/imports/{id}/rows/{statement}/{position}', {
			params: {
				path: { id, statement: row.statement, position: row.position },
				header: { 'If-Match': etag }
			},
			body
		});
		const updated = ok(r);
		rows = rows.map((x) => (rowKey(x) === rowKey(updated) ? updated : x));
		// A transfer pair's other row shares the choice, and the import's
		// counts follow: both come from the server.
		if ('decision' in body && updated.transfer_pair) await load();
		else {
			const i = await api.GET('/api/v1/imports/{id}', { params: { path: { id } } });
			imp = ok(i);
			etag = i.response.headers.get('etag') ?? '';
		}
		return updated;
	}

	/** Where a row is: its statement's account and commodity. */
	function bankOf(row: Row) {
		const s = imp?.statements.find((x) => x.position === row.statement);
		return s?.account_id ? { account: s.account_id, commodity: s.commodity_id } : undefined;
	}

	/** Edit: the row's form opens under it; again, it closes. */
	function openRow(row: Row) {
		const bank = bankOf(row);
		open = rowKey(row);
		const draft = bank && ref ? rowDraft(row, bank, ref) : null;
		form = draft ? { draft, tried: false, busy: false, failure: null } : null;
	}
	function toggle(row: Row) {
		if (open === rowKey(row)) {
			open = null;
			form = null;
		} else openRow(row);
	}

	/** Saving makes everything the form shows yours; what's still to review
	 * (a default short of certain) is ticked, so the row is ready. */
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
				const names = new Set([...r.tags.values()].map((t) => t.name));
				if (fresh.length || f.draft.tags.some((t) => !names.has(t))) forgetReference();
				if (fresh.length) r = ref = await reference();
				const saved = await patch(row, rowUpdate(row, $state.snapshot(f.draft), r));
				if (saved.state === 'to_review') await patch(saved, { reviewed: true });
				open = null;
				form = null;
			},
			(e) => (f.failure = e)
		);
		f.busy = false;
	}

	const matching = (s: State) => rows.filter((r) => r.state === s);
	function pin(s: State | '') {
		pinned = s ? new Set(matching(s).map(rowKey)) : null;
	}
	function show(s: State | '') {
		shown = s;
		pin(s);
	}

	/** The other side's name: an account (a transfer), a counterparty, or
	 * none (someone outside, unknown who). */
	function sideName(r: Reference, side: Side): string | null {
		if (!side) return null;
		if ('account' in side) {
			const a = r.accounts.get(side.account);
			return a ? accountLabel(a, r) : '?';
		}
		return side.counterparty ? (r.counterparties.get(side.counterparty)?.name ?? '?') : null;
	}

	function nameOf(r: Reference, label: Label, value: string | null | undefined): string | null {
		if (!value) return null;
		const named = label === 'category' ? r.categories.get(value) : r.parties.get(value);
		return named?.name ?? '?';
	}

	/** A label's value on screen, by name; `null` while it's unset. */
	function valueOf(r: Reference, row: Row, label: Label): string | null {
		if (row.missing.includes(label)) return null;
		if (label === 'counterparty') return sideName(r, sideOf(row));
		return nameOf(r, label, label === 'category' ? row.category_id : row.purpose_id);
	}

	async function commit() {
		busy = true;
		notice = '';
		asking = false;
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

	/** Commit; with rows to review left, ask once first. */
	function tryCommit(i: Schemas['Import']) {
		if (unreviewedQuestion(i.readiness)) asking = true;
		else commit();
	}

	// Suggestions, labels and transfer pairs worked out again; yours stay.
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

	const visible = $derived(pinned ? rows.filter((r) => pinned?.has(rowKey(r))) : rows);
	const toTick = $derived(visible.filter((r) => r.state === 'to_review'));
	/** What the tick-all box covers: the rows shown to review, and those
	 * ticked since. It's ticked once they all are. */
	const covered = $derived(visible.filter((r) => r.state === 'to_review' || r.reviewed));
	const allTicked = $derived(covered.length > 0 && covered.every((r) => r.reviewed));
	/** The open form's splits, for its Add split. */
	let lines: SplitLines | undefined = $state();
	const several = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
</script>

{#snippet block(state: State, title: string)}
	<Mark tone={states[state].tone} label={states[state].label} quiet />
	<span class="title">{title}</span>
{/snippet}

<p class="back"><a href={resolve('/imports')}>← Imports</a></p>

{#if result && ref}
	<h1 class="file">{name}</h1>
	<CommittedImport committed={result} {ref} />
{:else if imp && ref}
	{@const r = ref}
	{@const i = imp}
	{@const line = commitLine(i.readiness)}
	<h1 class="file">{name}</h1>
	<p class="muted uploaded">Uploaded {stampName(i.created_at)}</p>

	<Section label="Import" tag="section">
		{#snippet children(look)}
			{#each i.statements as s (s.position)}
				{@const code = r.commodities.get(s.commodity_id)?.code}
				{@const sg = s.suggested_account}
				<section class="block {states[s.state].tone}">
					<div class="head">
						{@render block(
							s.state,
							s.number_last4 ? `Statement …${s.number_last4}` : 'Statement, no account number'
						)}
					</div>
					<dl class="facts">
						<dt><label for="account-{s.position}">Account</label></dt>
						<dd>
							<select
								id="account-{s.position}"
								value={s.account_id ?? ''}
								onchange={(e) => setStatement(s, { account_id: e.currentTarget.value || null })}
							>
								{#if !s.account_id}<option value="" disabled>Choose…</option>{/if}
								{#each accountsByOwner(r).filter((a) => !a.archived || a.id === s.account_id) as a (a.id)}
									<option value={a.id}>{accountLabel(a, r)}</option>
								{/each}
							</select>
							{#if !s.account_id}<div class="under"><NoValue kind="unknown" /></div>{/if}
						</dd>
						<dd class="aside">
							{#if sg && s.account_suggested}<SuggestionBadge suggestion={sg} compact />{/if}
						</dd>
						{#if sg && (s.account_suggested || s.reviewed)}
							<dt></dt>
							<dd>
								<label class="choice">
									<input
										type="checkbox"
										checked={s.reviewed}
										onchange={(e) => setStatement(s, { reviewed: e.currentTarget.checked })}
									/>
									Reviewed
								</label>
							</dd>
							<dd class="aside"></dd>
						{/if}
						<dt>Rows</dt>
						<dd>
							{#if !s.account_id}
								<div>{s.rows.needs_you} wait for the account</div>
							{:else}
								{#each counted(s.rows) as c (c.state)}<div>{c.text}</div>{:else}<div>
										<NoValue kind="none" />
									</div>{/each}
							{/if}
						</dd>
						<dd class="aside"></dd>
						{#if s.ledger}
							<dt>Balance</dt>
							<dd><Amount value={s.ledger.amount} {code} /> on {dayName(s.ledger.date)}</dd>
							<dd class="aside"></dd>
						{/if}
						{#if s.skipped.length}
							<dt>Skipped</dt>
							<dd>
								<details>
									<summary>
										{several(s.skipped.length, "line couldn't be read", "lines couldn't be read")}
									</summary>
									<ul class="skipped">
										{#each s.skipped as l (l.line)}<li>Line {l.line}: {l.reason}</li>{/each}
									</ul>
								</details>
							</dd>
							<dd class="aside"></dd>
						{/if}
					</dl>
				</section>
			{:else}
				<p class="muted">The file has no statements.</p>
			{/each}

			<div class="filters" class:narrow={look === 'narrow'} role="group" aria-label="Rows to show">
				{#each stateOrder as st (st)}
					<button
						class="filter"
						aria-pressed={shown === st}
						onclick={() => (shown === st ? show('') : show(st))}
					>
						<Mark tone={states[st].tone} label={states[st].label} quiet />
						<span class="filter-name">{states[st].label}</span>
						<span class="n">{i.readiness.rows[st]}</span>
					</button>
				{/each}
				<button class="filter" aria-pressed={shown === ''} onclick={() => show('')}>
					<span class="filter-name all">All</span>
					<span class="n">{rows.length}</span>
				</button>
			</div>

			{#if notice}<p class="notice" role="status">{notice}</p>{/if}
			{#if failure}<Failure error={failure} />{/if}

			<div class="actions" class:narrow={look === 'narrow'}>
				<div class="commit">
					<button disabled={busy || line.blocked} onclick={() => tryCommit(i)}>Commit import</button
					>
					<span class="commit-line">
						{#if line.blocked}<Mark tone="error" label="Needs you" quiet />{/if}
						{line.text}
					</span>
				</div>
				<div class="others">
					<button class="quiet" disabled={busy || refreshing} onclick={refresh}
						>{refreshing ? 'Refreshing…' : 'Refresh suggestions'}</button
					>
					<button class="quiet danger" onclick={discard}>Discard import</button>
				</div>
			</div>
			{#if asking}
				<div class="ask" role="alertdialog" aria-label="Commit with rows to review">
					<p>{unreviewedQuestion(i.readiness)}</p>
					<div class="buttons">
						<button disabled={busy} onclick={commit}>Commit anyway</button>
						<button class="quiet" onclick={() => (asking = false)}>Cancel</button>
					</div>
				</div>
			{/if}
		{/snippet}
	</Section>

	<Section label="Rows" tag="section">
		{#snippet children(look)}
			<div class="rows">
				<div class="bar" class:narrow={look === 'narrow'}>
					<span>{several(toTick.length, 'row to review', 'rows to review')}</span>
					<label class="choice tick-all" class:off={!covered.length}>
						<input
							type="checkbox"
							checked={allTicked}
							disabled={!covered.length || busy}
							onchange={() => markReviewed(allTicked ? covered : toTick, !allTicked)}
						/>
						Reviewed: all {covered.length}
					</label>
				</div>
				{#each visible as row (rowKey(row))}
					{@const s = i.statements.find((x) => x.position === row.statement)}
					{@const code = s ? r.commodities.get(s.commodity_id)?.code : undefined}
					{@const tx = row.match_transaction_id ? txs.get(row.match_transaction_id) : undefined}
					{@const key = rowKey(row)}
					{@const adds = row.decision === 'add' || row.decision === 'add_separately'}
					{#snippet description()}
						<div>{row.set_description ?? row.description}</div>
						{#if row.set_description}<div class="muted">Bank: {row.description}</div>{/if}
					{/snippet}
					{#snippet amount()}<Amount value={row.amount} {code} />{/snippet}
					{#snippet status()}
						<RowStatus
							tone={states[row.state].tone}
							state={states[row.state].label}
							words={statusWords(row)}
						>
							{#if row.transfer_pair}
								{@const m = row.transfer_pair}
								{@const a = r.accounts.get(m.account_id)}
								{a ? accountLabel(a, r) : '?'}:
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- sideLink resolves -->
								<a href={sideLink(m)}
									>{dayName(m.date)}
									{m.description}
									<Amount value={negate(row.amount)} {code} /> ↗</a
								>
								{#if m.import_id !== i.id}(in {m.file_name ?? 'another import'}){/if}
							{:else if row.match_transaction_id && (row.status === 'duplicate' || row.status === 'possible_duplicate')}
								<a href={resolve('/transactions/[id]', { id: row.match_transaction_id })}
									>{#if tx}{dayName(tx.date)}
										{tx.description}
										{#if s?.account_id}<Quantities
												items={effectOn(tx, s.account_id, r)}
												inline
											/>{/if}
									{:else}A transaction{/if} ↗</a
								>
							{/if}
						</RowStatus>
					{/snippet}
					{#snippet values()}
						<Values>
							{#if row.status === 'no_account'}
								<dt>Account</dt>
								<dd><NoValue kind="unknown" /></dd>
								<dd class="aside"></dd>
							{/if}
							{#if adds && row.splits.length}
								{#each row.splits as p, k (k)}
									{@const side = p.transfer_account_id
										? { account: p.transfer_account_id }
										: { counterparty: p.counterparty_id ?? null }}
									<dt>Split {k + 1}</dt>
									<dd>
										{#if sideName(r, side)}{sideName(r, side)}{:else}<NoValue kind="unknown" />{/if}
										<Amount value={p.amount} {code} />
									</dd>
									<dd class="aside"></dd>
								{/each}
							{:else if adds}
								{#each labels as label (label)}
									<!-- Added as one transfer, the other side is the pair's: said in its status. -->
									{#if !row.not_applicable.includes(label) && !(label === 'counterparty' && row.transfer_pair && row.decision === 'add')}
										{@const st = labelState(row, label)}
										{@const v = valueOf(r, row, label)}
										<dt>{labelNames[label]}</dt>
										<dd>
											{#if v}{v}{:else}<NoValue kind="unknown" />{/if}
										</dd>
										<dd class="aside">
											{#if st.kind === 'suggested'}
												<SuggestionBadge suggestion={st.suggestion} compact />
											{/if}
										</dd>
									{/if}
								{/each}
							{/if}
							{#if adds && row.tags.length}
								<dt>Tags</dt>
								<dd>{row.tags.join(', ')}</dd>
								<dd class="aside"></dd>
							{/if}
						</Values>
					{/snippet}
					{#snippet actions()}
						{#if row.options.length}
							<fieldset class="options">
								<legend>On commit</legend>
								{#each row.options as d (d)}
									<label class="box">
										<input
											type="radio"
											name="row-{key}"
											checked={row.decision === d}
											onchange={() => setRow(row, { decision: d })}
										/>
										{choiceName(row.status, d)}
									</label>
								{/each}
							</fieldset>
						{:else}
							<p class="muted waits">Waits for the statement's account</p>
						{/if}
						<div class="tools">
							{#if row.state === 'to_review' || row.reviewed}
								<label class="box">
									<input
										type="checkbox"
										checked={row.reviewed}
										onchange={(e) => setRow(row, { reviewed: e.currentTarget.checked })}
									/>
									Reviewed
								</label>
							{/if}
							<button class="box" aria-expanded={open === key} onclick={() => toggle(row)}
								>Edit</button
							>
						</div>
					{/snippet}
					{#snippet rowForm()}
						<div id="form-{anchor(row)}">
							{#snippet facts()}
								{#if row.native_id}<span>Bank ID {row.native_id}</span>{/if}
								{#each Object.entries(row.raw as Record<string, unknown>) as [k, v] (k)}
									<span>{k} {typeof v === 'string' ? v : JSON.stringify(v)}</span>
								{/each}
							{/snippet}
							{#if form}
								{@const f = form}
								{@const bank = bankOf(row)}
								{@const one = (n: number) =>
									n === 0 && f.draft.splits.length === 1 && !row.splits.length}
								{@const paired = !!row.transfer_pair && row.decision === 'add'}
								{#snippet badge(n: number, field: SplitField)}
									{@const label = field === 'other' ? 'counterparty' : field}
									{@const st = labelState(row, label)}
									{#if one(n) && st.kind === 'suggested' && !(paired && label === 'counterparty')}
										<SuggestionBadge suggestion={st.suggestion} compact />
									{/if}
								{/snippet}
								{#snippet status(n: number, field: SplitField)}
									{#if one(n) && paired && field === 'other'}The transfer pair's other row{/if}
								{/snippet}
								{#snippet more()}
									{#if bank && adds && row.status !== 'transfer_pair'}
										<button
											type="button"
											class="quiet"
											onclick={() => lines?.add()}
											disabled={f.busy}>Add split</button
										>
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
									actions={more}
									{facts}
								>
									<Field label="Description" id="description-{anchor(row)}">
										<input
											id="description-{anchor(row)}"
											bind:value={f.draft.description}
											placeholder={row.description}
											maxlength="500"
										/>
									</Field>
									<Field label="Tags" id="tags-{anchor(row)}">
										<Tags
											bind:tags={f.draft.tags}
											known={sorted(r.tags).map((t) => t.name)}
											id="tags-{anchor(row)}"
											label="Tags"
										/>
									</Field>
									{#if bank}
										<SplitLines
											bind:this={lines}
											bind:splits={f.draft.splits}
											ref={r}
											commodity={bank.commodity}
											fixed={{ account: bank.account, side: fixedSide(row) }}
											expected={whole(row)}
											more={false}
											{badge}
											{status}
										/>
									{/if}
								</RowForm>
							{:else}
								<p class="raw muted">{@render facts()}</p>
							{/if}
						</div>
					{/snippet}
					<TransactionRow
						id="row-{anchor(row)}"
						label="{dayName(row.date)} {row.set_description ?? row.description}"
						date={row.date}
						tone={states[row.state].tone}
						flash={flashed.has(key)}
						muted={row.state === 'not_added'}
						{description}
						{amount}
						{status}
						{values}
						{actions}
						form={open === key ? rowForm : undefined}
					/>
				{:else}
					<p class="muted none">No rows.</p>
				{/each}
			</div>
		{/snippet}
	</Section>
{:else if failure}
	<Failure error={failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.back {
		margin: 0 0 var(--space-3);
	}
	.file {
		overflow-wrap: anywhere;
		margin-bottom: var(--space-1);
	}
	.uploaded {
		margin: 0 0 var(--space-4);
	}
	/* A statement with its colour: a strip down its left edge. */
	.block {
		--strip: var(--color-positive);
		margin-bottom: var(--space-3);
		padding: var(--space-3) var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-left: 4px solid var(--strip);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.block.error {
		--strip: var(--color-negative);
	}
	.block.warning {
		--strip: var(--color-warning);
	}
	.block.off {
		--strip: var(--color-smoke);
	}
	.head {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		margin-bottom: var(--space-2);
		font-weight: var(--weight-strong);
		overflow-wrap: anywhere;
	}
	/* One item per line: names in a column, values using the rest, badges
	   last. */
	.facts {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr) max-content;
		gap: var(--space-1) var(--space-3);
		align-items: baseline;
		margin: 0;
	}
	.facts dt {
		color: var(--color-text-muted);
	}
	.facts dd {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.facts select {
		width: 100%;
		max-width: var(--field-max);
	}
	.under {
		margin-top: var(--space-1);
	}
	.skipped {
		margin: var(--space-1) 0 0;
		padding-left: var(--space-4);
	}
	/* The filters: one row of equal buttons, or a list. */
	.filters {
		display: grid;
		grid-auto-flow: column;
		grid-auto-columns: minmax(max-content, 1fr);
		gap: var(--space-2);
		margin: var(--space-4) 0 var(--space-3);
	}
	.filters.narrow {
		grid-auto-flow: row;
		grid-auto-columns: auto;
		gap: 0;
	}
	.filter {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		white-space: nowrap;
		border-color: var(--color-border);
		background: transparent;
		color: var(--color-text);
		text-align: left;
	}
	.filters.narrow .filter {
		border-color: transparent;
	}
	.filter:hover {
		background: var(--color-surface-alt);
	}
	.filters .filter[aria-pressed='true'] {
		border-color: var(--color-accent);
		background: var(--color-surface);
		font-weight: var(--weight-strong);
	}
	.filter-name {
		flex: 1;
	}
	.filter-name.all {
		padding-left: calc(1ch + var(--space-2));
	}
	.n {
		text-align: right;
		font-variant-numeric: var(--numeric);
	}
	/* The actions: Commit and its line on the left, the rest on the right; or
	   all stacked. Commit never leaves its line. */
	.actions {
		display: flex;
		justify-content: space-between;
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}
	.actions.narrow {
		flex-direction: column;
		align-items: flex-start;
	}
	.commit,
	.others {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		white-space: nowrap;
	}
	.actions.narrow .commit {
		white-space: normal;
	}
	.actions.narrow .others {
		flex-direction: column;
		align-items: flex-start;
	}
	.ask {
		margin-bottom: var(--space-4);
		padding: var(--space-3) var(--space-4);
		border-left: 4px solid var(--color-warning);
		border-radius: var(--radius-sm);
		background: var(--color-warning-bg);
	}
	.ask p {
		margin: 0 0 var(--space-3);
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
	}
	.danger {
		color: var(--color-negative);
	}
	.notice {
		margin: 0 0 var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-sm);
		background: var(--color-surface-alt);
	}
	.rows {
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	/* Over the rows: how many are to review, and the box that ticks them all,
	   in the column of the rows' own Reviewed boxes. */
	.bar {
		display: grid;
		grid-template-columns: minmax(0, 1fr) var(--row-actions);
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-3) var(--space-2) calc(var(--space-3) + 4px);
		border-bottom: var(--border-width) solid var(--color-border);
		background: var(--color-surface-alt);
		border-radius: var(--radius-md) var(--radius-md) 0 0;
	}
	.bar.narrow {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
	}
	.tick-all {
		padding: 0 var(--space-2);
	}
	.tick-all.off {
		color: var(--color-text-muted);
		cursor: default;
	}
	.none {
		margin: 0;
		padding: var(--space-3);
	}
	.options {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		border: none;
	}
	.options legend {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
	.waits {
		margin: 0;
	}
	.raw {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		margin: 0;
		font-size: var(--text-xs);
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}
</style>
