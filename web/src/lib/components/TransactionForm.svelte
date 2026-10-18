<script lang="ts">
	// One transaction's form, opened in place: under its row to edit it, or at
	// the top of the list to add one (blank, or like an existing one).
	import { usd } from '../usd';
	import { resolve } from '$app/paths';
	import { api, done, ok, type Schemas } from '../api/client';
	import Failure from './Failure.svelte';
	import RowForm from './RowForm.svelte';
	import SplitLines from './SplitLines.svelte';
	import { today } from '../dates';
	import { forgetReference, reference, sorted, type Reference } from '../reference';
	import {
		blankDraft,
		fromTransaction,
		newCounterparties,
		problems,
		toRequest,
		type Draft
	} from '../views/editor';

	type Transaction = Schemas['Transaction'];

	let {
		id,
		copy,
		account,
		onclose,
		oncopy
	}: {
		/** The transaction to edit, or `null` for a new one. */
		id: string | null;
		/** A new one: like this transaction, dated today. */
		copy?: string;
		/** A new one: from this account. */
		account?: string;
		/** Closed: saved as `saved`, deleted, or neither (Cancel). */
		onclose: (change?: { saved?: Transaction; deleted?: true }) => void;
		/** Copy as new, asked from an existing transaction. */
		oncopy?: (id: string) => void;
	} = $props();
	let creating = $derived(id === null);
	const uid = $props.id();

	let ref: Reference | null = $state(null);
	let draft: Draft | null = $state(null);
	let tx: Transaction | null = $state(null);
	let etag = $state('');
	// Shown from the first save on, and updated as the draft changes.
	let tried = $state(false);
	let issues = $derived(tried && draft && ref ? problems(draft, ref) : []);
	let failure: unknown = $state(null);
	let busy = $state(false);

	const household = (r: Reference) => usd(r)?.id ?? [...r.commodities.keys()][0] ?? '';

	async function get(id: string) {
		const r = await api.GET('/api/v1/transactions/{id}', { params: { path: { id } } });
		return { tx: ok(r), etag: r.response.headers.get('etag') ?? '' };
	}

	async function load() {
		failure = null;
		draft = null;
		tried = false;
		try {
			const r = await reference();
			if (id) {
				const got = await get(id);
				etag = got.etag;
				tx = got.tx;
				draft = fromTransaction(got.tx, r);
			} else if (copy) {
				const d = fromTransaction((await get(copy)).tx, r);
				draft = { ...d, date: today(), externalRefs: [] };
			} else {
				const d = blankDraft(household(r), today());
				if (account && r.accounts.has(account)) d.splits[0].from.account = account;
				draft = d;
			}
			ref = r;
		} catch (e) {
			failure = e;
		}
	}
	load();

	async function save() {
		if (!draft || !ref) return;
		tried = true;
		if (problems(draft, ref).length) return;
		busy = true;
		failure = null;
		try {
			let r = ref;
			const fresh = newCounterparties(draft, r);
			for (const name of fresh) ok(await api.POST('/api/v1/counterparties', { body: { name } }));
			if (fresh.length) {
				forgetReference();
				r = ref = await reference();
			}
			const body = toRequest(draft, r);
			const saved = id
				? ok(
						await api.PUT('/api/v1/transactions/{id}', {
							params: { path: { id }, header: { 'If-Match': etag } },
							body
						})
					)
				: ok(await api.POST('/api/v1/transactions', { body }));
			const names = new Set([...r.tags.values()].map((t) => t.name));
			if (body.tags?.some((t) => !names.has(t))) forgetReference();
			onclose({ saved });
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	async function remove() {
		if (!id || !draft || !confirm(`Delete “${draft.description}”? This can't be undone.`)) return;
		busy = true;
		failure = null;
		try {
			done(
				await api.DELETE('/api/v1/transactions/{id}', {
					params: { path: { id }, header: { 'If-Match': etag } }
				})
			);
			onclose({ deleted: true });
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}
</script>

{#if draft && ref}
	<RowForm
		label={creating ? 'Add transaction' : `Edit ${tx?.description ?? 'transaction'}`}
		{busy}
		{issues}
		{failure}
		onsave={save}
		oncancel={() => onclose()}
		onreload={load}
	>
		<div class="fields">
			<label class="field">
				<span>Date</span>
				<input type="date" bind:value={draft.date} />
			</label>
			<label class="field grow">
				<span>Description</span>
				<input bind:value={draft.description} maxlength="500" />
			</label>
		</div>
		<div class="fields">
			<label class="field grow">
				<span>Tags, comma-separated</span>
				<input bind:value={draft.tags} list="{uid}-tags" />
			</label>
			<datalist id="{uid}-tags">
				{#each sorted(ref.tags) as t (t.id)}<option value={t.name}></option>{/each}
			</datalist>
			<label class="field grow">
				<span>Notes</span>
				<input bind:value={draft.notes} />
			</label>
		</div>
		<SplitLines bind:splits={draft.splits} {ref} commodity={household(ref)} />

		{#snippet actions()}
			{#if id && oncopy}
				<button type="button" class="quiet" onclick={() => id && oncopy(id)} disabled={busy}
					>Copy as new</button
				>
			{/if}
			{#if id}
				<button type="button" class="quiet danger" onclick={remove} disabled={busy}>Delete</button>
			{/if}
		{/snippet}
		{#snippet facts()}
			{#if tx}
				{#each tx.external_refs as e (e.source)}<span>{e.source} {e.ref}</span>{/each}
				<span>id {tx.id}</span>
				<span
					>import batch <a href={resolve(`/transactions?import_batch_id=${tx.import_batch_id}`)}
						>{tx.import_batch_id}</a
					></span
				>
			{/if}
		{/snippet}
	</RowForm>
{:else if failure}
	<Failure error={failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}
