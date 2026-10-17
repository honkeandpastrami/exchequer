<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { all, api, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { localDate } from '#lib/dates.ts';
	import { accountLabel } from '#lib/names.ts';
	import { reference, type Reference } from '#lib/reference.ts';
	import {
		committedSummary,
		readinessNames,
		readinessWhy,
		statementName
	} from '#lib/views/imports.ts';

	let ref: Reference | null = $state(null);
	let staged: Schemas['Import'][] | null = $state(null);
	let failures: { file: string; error: unknown }[] = $state([]);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let input: HTMLInputElement | undefined = $state();
	/** What the last commit did, a line per file. */
	let notices: string[] = $state([]);
	let committing = $state(false);

	const ready = $derived.by(() => staged?.filter((i) => i.readiness.state === 'ready') ?? []);
	const name = (i: { file_name?: string | null; id: string }) => i.file_name ?? 'A file';

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

	/** One import, as it is now: refused if it needs work since. */
	async function commitOne(imp: Schemas['Import']) {
		committing = true;
		notices = [];
		failure = null;
		try {
			const r = await api.GET('/api/v1/imports/{id}', { params: { path: { id: imp.id } } });
			const now = ok(r);
			if (now.readiness.state === 'needs_work') {
				notices = [`${name(imp)}: needs work since it was listed.`];
			} else {
				const c = await api.POST('/api/v1/imports/{id}/commit', {
					params: {
						path: { id: imp.id },
						header: { 'If-Match': r.response.headers.get('etag') ?? '' }
					}
				});
				notices = [
					`${name(imp)}: ${c.data ? committedSummary(c.data) : (c.error?.detail ?? c.error?.title ?? 'not committed')}.`
				];
			}
		} catch (e) {
			failure = e;
		}
		committing = false;
		await load();
	}

	async function commitReady() {
		committing = true;
		notices = [];
		failure = null;
		try {
			const r = ok(
				await api.POST('/api/v1/imports/commit-ready', {
					body: { ids: ready.map((i) => i.id) }
				})
			);
			notices = r.results.map(
				(o) =>
					`${name(o)}: ${o.committed ? committedSummary(o.committed) : `not committed, ${o.skipped}`}.`
			);
		} catch (e) {
			failure = e;
		}
		committing = false;
		await load();
	}
</script>

<h1>Imports</h1>
<p class="muted intro">
	Upload bank files (OFX, QFX, QBO or CSV). Nothing enters the ledger until you review an import and
	commit it; until then it stays here, for any client to finish.
</p>

<label class="upload">
	<span>Upload files</span>
	<input
		bind:this={input}
		type="file"
		multiple
		accept=".ofx,.qfx,.qbo,.csv,.txt"
		disabled={busy}
		onchange={(e) => e.currentTarget.files?.length && upload(e.currentTarget.files)}
	/>
	{#if busy}<span class="muted">Uploading…</span>{/if}
</label>
<button
	class="commit-ready"
	disabled={committing || busy || ready.length === 0}
	onclick={commitReady}>Commit {ready.length} ready</button
>
{#each notices as n, i (i)}<p class="notice" role="status">{n}</p>{/each}

{#each failures as f (f.file)}
	<div class="failed"><strong>{f.file}</strong><Failure error={f.error} /></div>
{/each}
{#if failure}<Failure error={failure} />{/if}

{#if ref && staged}
	{@const r = ref}
	<div class="list">
		<table>
			<thead>
				<tr>
					<th>File</th>
					<th>Uploaded</th>
					<th>Statements</th>
					<th>Status</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each staged as imp (imp.id)}
					<tr>
						<td>
							<a href={resolve('/imports/[id]', { id: imp.id })}>{imp.file_name ?? imp.format}</a>
						</td>
						<td>{localDate(imp.created_at)}</td>
						<td>
							{#each imp.statements as s (s.position)}
								{@const a = s.account_id ? r.accounts.get(s.account_id) : undefined}
								<div>
									{statementName(s)} →
									{#if a}{accountLabel(a, r)}{:else}<span class="warn">no account yet</span>{/if}
								</div>
							{:else}
								<span class="muted">none</span>
							{/each}
						</td>
						<td>
							<span class="mark {imp.readiness.state}"
								><span class="dot" aria-hidden="true"></span>{readinessNames[
									imp.readiness.state
								]}</span
							>
							{#each readinessWhy(imp.readiness) as why (why)}
								<div class="why">{why}</div>
							{/each}
						</td>
						<td>
							<button
								disabled={committing || busy || imp.readiness.state === 'needs_work'}
								onclick={() => commitOne(imp)}>Commit</button
							>
						</td>
					</tr>
				{:else}
					<tr><td colspan="5" class="muted">Nothing staged.</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
{:else if !failure}
	<p class="muted">Loading…</p>
{/if}

<style>
	.intro {
		font-size: var(--text-sm);
		max-width: 50rem;
	}
	.upload {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}
	.failed {
		font-size: var(--text-sm);
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
	.warn {
		color: var(--color-warning);
	}
	.commit-ready {
		margin-bottom: var(--space-4);
	}
	.notice {
		font-size: var(--text-sm);
	}
	.mark {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		white-space: nowrap;
	}
	.dot {
		width: 0.7em;
		height: 0.7em;
		border-radius: 50%;
		background: currentColor;
	}
	.ready .dot {
		color: var(--color-positive);
	}
	.suggested .dot {
		color: var(--color-warning);
	}
	.needs_work .dot {
		color: var(--color-negative);
	}
	.why {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
</style>
