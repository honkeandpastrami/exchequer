<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { ApiError, api, done, ok } from '../api/client';
	import Failure from './Failure.svelte';
	import SplitEditor from './SplitEditor.svelte';
	import { today } from '../dates';
	import { hashQuery } from '../query';
	import { forgetReference, reference, sorted, type Reference } from '../reference';
	import { returnTo } from '../returnTo.svelte';
	import {
		blankDraft,
		blankSplit,
		fromTransaction,
		newCounterparties,
		problems,
		toRequest,
		type Draft
	} from '../views/editor';

	// The transaction to edit, or `null` for a new one.
	let { id }: { id: string | null } = $props();
	let creating = $derived(id === null);

	let ref: Reference | null = $state(null);
	let draft: Draft | null = $state(null);
	let etag = $state('');
	// Shown from the first save on, and updated as the draft changes.
	let tried = $state(false);
	let issues = $derived(tried && draft && ref ? problems(draft, ref) : []);
	let failure: unknown = $state(null);
	let busy = $state(false);

	const household = (r: Reference) =>
		[...r.parties.values()].find((p) => p.kind === 'household')?.currency_id ??
		[...r.commodities.keys()][0] ??
		'';

	async function get(id: string) {
		const r = await api.GET('/api/v1/transactions/{id}', { params: { path: { id } } });
		return { tx: ok(r), etag: r.response.headers.get('etag') ?? '' };
	}

	async function load(id: string | null, query: URLSearchParams) {
		failure = null;
		draft = null;
		tried = false;
		try {
			const r = await reference();
			const copy = query.get('copy');
			if (id) {
				const got = await get(id);
				etag = got.etag;
				draft = fromTransaction(got.tx, r);
			} else if (copy) {
				// A new transaction like an existing one, dated today.
				const d = fromTransaction((await get(copy)).tx, r);
				draft = { ...d, date: today(), externalRefs: [] };
			} else {
				const d = blankDraft(household(r), today());
				const account = query.get('account');
				if (account && r.accounts.has(account)) d.splits[0].from.account = account;
				draft = d;
			}
			ref = r;
		} catch (e) {
			failure = e;
		}
	}

	$effect(() => {
		load(id, hashQuery(page.url));
	});

	const back = () => goto(resolve(returnTo.transactions as '/transactions'));

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
			if (creating) ok(await api.POST('/api/v1/transactions', { body }));
			else if (id)
				ok(
					await api.PUT('/api/v1/transactions/{id}', {
						params: { path: { id }, header: { 'If-Match': etag } },
						body
					})
				);
			const names = new Set([...r.tags.values()].map((t) => t.name));
			if (body.tags?.some((t) => !names.has(t))) forgetReference();
			back();
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
			back();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}
</script>

{#if draft && ref}
	<form
		onsubmit={(e) => {
			e.preventDefault();
			save();
		}}
	>
		<datalist id="counterparties">
			{#each sorted(ref.counterparties) as c (c.id)}<option value={c.name}></option>{/each}
		</datalist>
		<div class="head">
			<label class="field">
				<span>Date</span>
				<input type="date" bind:value={draft.date} />
			</label>
			<label class="field grow">
				<span>Description</span>
				<input bind:value={draft.description} maxlength="500" />
			</label>
		</div>
		<div class="head">
			<label class="field grow">
				<span>Tags, comma-separated</span>
				<input bind:value={draft.tags} list="tags" />
			</label>
			<datalist id="tags">
				{#each sorted(ref.tags) as t (t.id)}<option value={t.name}></option>{/each}
			</datalist>
			<label class="field grow">
				<span>Notes</span>
				<input bind:value={draft.notes} />
			</label>
		</div>

		{#each draft.splits, i (i)}
			<SplitEditor
				bind:split={draft.splits[i]}
				{ref}
				title={draft.splits.length > 1 ? `Split ${i + 1}` : 'Split'}
				onremove={draft.splits.length > 1 ? () => draft?.splits.splice(i, 1) : undefined}
			/>
		{/each}
		<p>
			<button
				type="button"
				class="quiet"
				onclick={() => draft?.splits.push(blankSplit(household(ref!), draft.splits.at(-1)))}
				>Add split</button
			>
		</p>

		{#if draft.externalRefs.length}
			<p class="muted refs">
				Imported as {draft.externalRefs.map((e) => `${e.source} ${e.ref}`).join(', ')}
			</p>
		{/if}

		{#if issues.length}
			<ul class="issues" role="alert">
				{#each issues as m (m)}<li>{m}</li>{/each}
			</ul>
		{/if}
		{#if failure}
			<Failure error={failure} />
			{#if failure instanceof ApiError && failure.status === 412}
				<button type="button" class="quiet" onclick={() => load(id, hashQuery(page.url))}
					>Reload, discarding my changes</button
				>
			{/if}
		{/if}

		<div class="actions">
			<button type="submit" disabled={busy}>{busy ? 'Saving…' : 'Save'}</button>
			<button type="button" class="quiet" onclick={back} disabled={busy}>Cancel</button>
			{#if !creating}
				<a class="copy" href={resolve(`/transactions/new?copy=${id}`)}>Copy as new</a>
				<button type="button" class="quiet danger" onclick={remove} disabled={busy}>Delete</button>
			{/if}
		</div>
	</form>
{:else if failure}
	<Failure error={failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	form {
		max-width: 60rem;
	}
	.head {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.field > span {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.grow {
		flex: 1;
		min-width: 14rem;
	}
	.refs {
		font-size: var(--text-sm);
	}
	.issues {
		padding: var(--space-3) var(--space-5);
		border-radius: var(--radius-sm);
		background: var(--color-warning-bg);
		color: var(--color-warning);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-top: var(--space-4);
	}
	.copy {
		margin-left: auto;
		font-size: var(--text-sm);
	}
	.danger {
		color: var(--color-negative);
		border-color: var(--color-negative);
	}
</style>
