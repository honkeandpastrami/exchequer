<script lang="ts">
	// A month's state, its parameters (stipends and funders) and closing or
	// reopening it (design §4.3 and §4.4). Every action sends the ETag of the
	// last read, so an edit made elsewhere meanwhile asks for a reload.
	import { api, ApiError, ok, type Schemas } from '../api/client';
	import { monthName, stampName } from '../dates';
	import type { Reference } from '../reference';
	import {
		businesses,
		currency,
		draftFrom,
		household,
		paramProblems,
		people,
		toParams,
		unsaved,
		type ParamsDraft
	} from '../views/settlement-params';
	import Failure from './Failure.svelte';
	import Section from './Section.svelte';

	let { month, ref, onchange }: { month: string; ref: Reference; onchange: () => void } = $props();

	let current = $state<{ s: Schemas['Settlement']; etag: string } | null>(null);
	let draft: ParamsDraft = $state({ stipends: {}, funders: {} });
	let failure: unknown = $state(null);
	let busy = $state(false);
	let tried = $state(false);

	const path = (etag: string) => ({ params: { path: { month }, header: { 'If-Match': etag } } });
	const etagOf = (r: { response: Response }) => r.response.headers.get('etag') ?? '';

	async function load() {
		failure = null;
		tried = false;
		try {
			const r = await api.GET('/api/v1/settlements/{month}', { params: { path: { month } } });
			current = { s: ok(r), etag: etagOf(r) };
			draft = draftFrom(current.s, ref);
		} catch (e) {
			failure = e;
		}
	}
	$effect(() => {
		void month;
		load();
	});

	let issues = $derived(tried ? paramProblems(draft, ref) : []);
	let pending = $derived(current ? unsaved(draft, current.s, ref) : false);
	let code = $derived(currency(ref)?.code ?? '');
	let funderChoices = $derived([household(ref), ...people(ref)].filter((p) => p !== undefined));

	async function act(steps: (etag: string) => Promise<{ s: Schemas['Settlement']; etag: string }>) {
		if (!current) return;
		busy = true;
		failure = null;
		try {
			current = await steps(current.etag);
			draft = draftFrom(current.s, ref);
			tried = false;
			onchange();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	const save = async (etag: string) => {
		const r = await api.PUT('/api/v1/settlements/{month}', {
			...path(etag),
			body: toParams(draft, ref)
		});
		return { s: ok(r), etag: etagOf(r) };
	};
	const post = (what: 'close' | 'reopen') => async (etag: string) => {
		const r = await (what === 'close'
			? api.POST('/api/v1/settlements/{month}/close', path(etag))
			: api.POST('/api/v1/settlements/{month}/reopen', path(etag)));
		return { s: ok(r), etag: etagOf(r) };
	};

	function submit(closing: boolean) {
		tried = true;
		if (paramProblems(draft, ref).length) return;
		act(async (etag) => {
			const saved = pending ? await save(etag) : null;
			return closing ? post('close')(saved?.etag ?? etag) : (saved ?? current!);
		});
	}
</script>

{#if current}
	{@const s = current.s}
	<form
		class="card"
		onsubmit={(e) => {
			e.preventDefault();
			submit(false);
		}}
	>
		<Section label="Parameters">
			{#snippet children(look)}
				{@const narrow = look === 'narrow'}
				<div class="state" class:narrow>
					<strong>{s.closed ? 'Closed' : 'Open'}</strong>
					<span class="muted">
						{#if s.closed}
							{s.closed_at ? `since ${stampName(s.closed_at)}` : ''}
						{:else if s.params_from && s.params_from !== month}
							Parameters carried forward from {monthName(s.params_from)}
						{:else if !s.params_from}
							No parameters set yet.
						{/if}
					</span>
				</div>

				<fieldset disabled={s.closed || busy}>
					<div class="params" class:narrow>
						{#each people(ref) as p (p.id)}
							<label class="field">
								<span>Stipend for {p.name}</span>
								<span class="money">
									<input
										class="qty"
										inputmode="decimal"
										bind:value={draft.stipends[p.id]}
										placeholder="0.00"
									/>
									<span class="muted">{code}</span>
								</span>
							</label>
						{/each}
						{#each businesses(ref) as b (b.id)}
							<label class="field">
								<span>{b.name} funded by</span>
								<select bind:value={draft.funders[b.id]}>
									{#each funderChoices as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
								</select>
							</label>
						{/each}
					</div>
				</fieldset>

				{#if issues.length}
					<ul class="issues" role="alert">
						{#each issues as m (m)}<li>{m}</li>{/each}
					</ul>
				{/if}
				{#if failure}
					<Failure error={failure} />
					{#if failure instanceof ApiError && failure.status === 412}
						<button type="button" class="quiet" onclick={load}>Reload, discarding my changes</button
						>
					{/if}
				{/if}

				<div class="actions" class:narrow>
					{#if s.closed}
						<button type="button" onclick={() => act(post('reopen'))} disabled={busy}>Reopen</button
						>
					{:else}
						<button type="button" onclick={() => submit(true)} disabled={busy}>
							{pending ? 'Save and close' : 'Close'}
							{monthName(month)}
						</button>
						<button type="submit" class="quiet" disabled={busy || !pending}>Save parameters</button>
						{#if pending && s.params_from === month}
							<button type="button" class="quiet" onclick={() => (draft = draftFrom(s, ref))}
								>Undo changes</button
							>
						{/if}
					{/if}
				</div>
			{/snippet}
		</Section>
	</form>
{:else if failure}
	<Failure error={failure} />
{/if}

<style>
	.card {
		margin-bottom: var(--space-5);
		padding: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	/* Each row: one line while it fits, one item per line once it doesn't. */
	.state,
	.params,
	.actions {
		display: flex;
	}
	.state > :global(*),
	.params > :global(*),
	.actions > :global(*) {
		flex: none;
	}
	.narrow {
		flex-direction: column;
		align-items: start;
	}
	.state {
		align-items: baseline;
		gap: var(--space-3);
		font-size: var(--text-sm);
	}
	fieldset {
		margin: 0;
		padding: 0;
		border: 0;
		min-width: 0;
	}
	.params {
		gap: var(--space-3) var(--space-5);
		margin-top: var(--space-3);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.field > span:first-child {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.money {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.qty {
		width: 9rem;
		text-align: right;
		font-variant-numeric: var(--numeric);
	}
	.issues {
		margin: var(--space-3) 0 0;
		padding: var(--space-3) var(--space-5);
		border-radius: var(--radius-sm);
		background: var(--color-warning-bg);
		color: var(--color-warning);
	}
	.actions {
		gap: var(--space-3);
		margin-top: var(--space-4);
	}
</style>
