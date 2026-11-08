<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { all, api, ok, type Schemas } from '#lib/api/client.ts';
	import CommittedImport from '#lib/components/CommittedImport.svelte';
	import Failure from '#lib/components/Failure.svelte';
	import Mark from '#lib/components/Mark.svelte';
	import NoValue from '#lib/components/NoValue.svelte';
	import { localDate } from '#lib/dates.ts';
	import { accountLabel } from '#lib/names.ts';
	import { reference, type Reference } from '#lib/reference.ts';
	import { counted, states, unreviewedQuestion } from '#lib/views/imports.ts';

	let ref: Reference | null = $state(null);
	let staged: Schemas['Import'][] | null = $state(null);
	let failures: { file: string; error: unknown }[] = $state([]);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let input: HTMLInputElement | undefined = $state();
	/** What the last commit did, per file. */
	let outcomes: { name: string; committed?: Schemas['Committed']; skipped?: string }[] = $state([]);
	let committing = $state(false);
	/** The import whose Commit asks about its rows to review. */
	let asking: string | null = $state(null);

	const ready = $derived.by(() => staged?.filter((i) => i.readiness.state === 'ready') ?? []);
	const name = (i: { file_name?: string | null; format?: string }) =>
		i.file_name ?? (i.format ? `${i.format.toUpperCase()} file` : 'A file');

	async function load() {
		try {
			[ref, staged] = await Promise.all([
				reference(),
				all((query) => api.GET('/api/v1/imports', { params: { query } }))
			]);
		} catch (e) {
			failure = e;
		}
	}
	load();

	async function upload(list: FileList) {
		// A copy: clearing the input empties `list`.
		const files = [...list];
		busy = true;
		failures = [];
		const added: string[] = [];
		for (const f of files) {
			try {
				const imp = ok(
					await api.POST('/api/v1/imports', {
						params: { query: { file_name: f.name } },
						body: f as unknown as string,
						bodySerializer: (b) => b,
						headers: { 'Content-Type': 'application/octet-stream' }
					})
				);
				added.push(imp.id);
			} catch (e) {
				failures.push({ file: f.name, error: e });
			}
		}
		busy = false;
		if (input) input.value = '';
		// One file: straight to its review.
		if (added.length === 1 && files.length === 1) {
			await goto(resolve('/imports/[id]', { id: added[0] }));
			return;
		}
		await load();
	}

	/** One import, as it is now; the server refuses it if it needs you since. */
	async function commitOne(imp: Schemas['Import']) {
		committing = true;
		asking = null;
		outcomes = [];
		failure = null;
		try {
			const r = await api.GET('/api/v1/imports/{id}', { params: { path: { id: imp.id } } });
			ok(r);
			const c = await api.POST('/api/v1/imports/{id}/commit', {
				params: {
					path: { id: imp.id },
					header: { 'If-Match': r.response.headers.get('etag') ?? '' }
				}
			});
			outcomes = [
				c.data
					? { name: name(imp), committed: c.data }
					: { name: name(imp), skipped: c.error?.detail ?? c.error?.title ?? 'refused' }
			];
		} catch (e) {
			failure = e;
		}
		committing = false;
		await load();
	}

	/** Commit; with rows to review left, ask once first. */
	function tryCommit(imp: Schemas['Import']) {
		if (unreviewedQuestion(imp.readiness)) asking = imp.id;
		else commitOne(imp);
	}

	async function commitReady() {
		committing = true;
		asking = null;
		outcomes = [];
		failure = null;
		try {
			const r = ok(
				await api.POST('/api/v1/imports/commit-ready', {
					body: { ids: ready.map((i) => i.id) }
				})
			);
			outcomes = r.results.map((o) => ({
				name: name(o),
				committed: o.committed ?? undefined,
				skipped: o.skipped ?? undefined
			}));
		} catch (e) {
			failure = e;
		}
		committing = false;
		await load();
	}

	const several = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
</script>

<h1>Imports</h1>

<div class="top">
	<label class="upload">
		<span class="button">{busy ? 'Uploading…' : 'Upload files'}</span>
		<input
			bind:this={input}
			type="file"
			multiple
			accept=".ofx,.qfx,.qbo,.csv,.txt"
			disabled={busy}
			onchange={(e) => e.currentTarget.files?.length && upload(e.currentTarget.files)}
		/>
	</label>
	<button disabled={committing || busy || ready.length === 0} onclick={commitReady}
		>Commit {several(ready.length, 'ready import', 'ready imports')}</button
	>
</div>

{#each failures as f (f.file)}
	<div class="failed"><strong>{f.file}</strong><Failure error={f.error} /></div>
{/each}
{#if failure}<Failure error={failure} />{/if}

{#if ref}
	{#each outcomes as o, k (k)}
		<h2 class="file">{o.name}</h2>
		{#if o.committed}
			<CommittedImport committed={o.committed} {ref} />
		{:else}
			<p class="notice" role="status">Not committed: {o.skipped}</p>
		{/if}
	{/each}
{/if}

{#if ref && staged}
	{@const r = ref}
	<div class="list">
		{#each staged as imp (imp.id)}
			{@const st = imp.readiness.state}
			<section class="block {st}">
				<div class="head">
					<span class="title">
						<Mark tone={states[st].tone} label={states[st].label} quiet />
						{name(imp)}
					</span>
					<a class="go" href={resolve('/imports/[id]', { id: imp.id })}>Review ▸</a>
				</div>
				<dl class="facts">
					<dt>Uploaded</dt>
					<dd>{localDate(imp.created_at)}</dd>
					<dd class="aside">
						{#if st !== 'needs_you'}
							<button disabled={committing || busy} onclick={() => tryCommit(imp)}>Commit</button>
						{/if}
					</dd>
					<dt>Statements</dt>
					<dd>
						{#each imp.statements as s (s.position)}
							{@const a = s.account_id ? r.accounts.get(s.account_id) : undefined}
							<div class="statement">
								{#if s.number_last4}<span class="number">…{s.number_last4}</span>{/if}
								{#if a}{accountLabel(a, r)}{:else}<NoValue kind="unknown" />{/if}
							</div>
						{:else}
							<NoValue kind="none" />
						{/each}
					</dd>
					<dd class="aside"></dd>
					<dt>Rows</dt>
					<dd>
						{#each counted(imp.readiness.rows) as c (c.state)}<div>{c.text}</div>{:else}<NoValue
								kind="none"
							/>{/each}
					</dd>
					<dd class="aside"></dd>
				</dl>
				{#if asking === imp.id}
					<div class="ask" role="alertdialog" aria-label="Commit with rows to review">
						<p>{unreviewedQuestion(imp.readiness)}</p>
						<div class="buttons">
							<button disabled={committing} onclick={() => commitOne(imp)}>Commit anyway</button>
							<button class="quiet" onclick={() => (asking = null)}>Cancel</button>
						</div>
					</div>
				{/if}
			</section>
		{:else}
			<p class="muted">Nothing staged.</p>
		{/each}
	</div>
{:else if !failure}
	<p class="muted">Loading…</p>
{/if}

<style>
	.top {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}
	/* The file input itself is hidden; its label is the button. */
	.upload input {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}
	.upload .button {
		display: inline-flex;
		align-items: center;
		min-height: var(--target);
		padding: var(--space-2) var(--space-4);
		border: var(--border-width) solid var(--color-accent);
		border-radius: var(--radius-sm);
		background: var(--color-accent);
		color: var(--color-on-accent);
		cursor: pointer;
	}
	.upload:focus-within .button {
		outline: 2px solid var(--color-focus);
		outline-offset: 2px;
	}
	.failed {
		font-size: var(--text-sm);
	}
	.file {
		overflow-wrap: anywhere;
	}
	.notice {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-sm);
		background: var(--color-surface-alt);
	}
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
	.needs_you {
		--strip: var(--color-negative);
	}
	.to_review {
		--strip: var(--color-warning);
	}
	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		margin-bottom: var(--space-1);
		font-weight: var(--weight-strong);
	}
	.title {
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.go {
		display: inline-flex;
		align-items: center;
		min-height: var(--target);
		white-space: nowrap;
		font-weight: var(--weight-normal);
	}
	/* One item per line: names in a column, values using the rest. */
	.facts {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr) max-content;
		gap: var(--space-1) var(--space-3);
		align-items: baseline;
		margin: 0;
		font-size: var(--text-sm);
	}
	dt {
		color: var(--color-text-muted);
	}
	dd {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.aside {
		justify-self: end;
	}
	.number {
		margin-right: var(--space-2);
		font-variant-numeric: var(--numeric);
	}
	.ask {
		margin-top: var(--space-3);
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
	/* A phone: names above their values. */
	@media (max-width: 40rem) {
		.facts {
			grid-template-columns: minmax(0, 1fr) max-content;
		}
		dt {
			grid-column: 1 / -1;
		}
	}
</style>
