<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { all, api, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { localDate } from '#lib/dates.ts';
	import { accountLabel } from '#lib/names.ts';
	import { reference, type Reference } from '#lib/reference.ts';
	import { counted, statementName, statuses, totals } from '#lib/views/imports.ts';

	let ref: Reference | null = $state(null);
	let staged: Schemas['Import'][] | null = $state(null);
	let failures: { file: string; error: unknown }[] = $state([]);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let input: HTMLInputElement | undefined = $state();

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
					<th>Rows</th>
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
							{#each counted(totals(imp.statements)) as [status, n] (status)}
								<div>{n} {statuses[status].toLowerCase()}</div>
							{/each}
						</td>
					</tr>
				{:else}
					<tr><td colspan="4" class="muted">Nothing staged.</td></tr>
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
</style>
