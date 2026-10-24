<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { localDate } from '#lib/dates.ts';

	type Invite = Schemas['Invite'];

	let users: Schemas['User'][] | null = $state(null);
	let me: string | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let adding: string | null = $state(null);
	/** The newest invite link per user, shown until the page is left. */
	let invites: Record<string, Invite> = $state({});

	async function load() {
		try {
			const [list, who] = await Promise.all([
				api.GET('/api/v1/users').then(ok),
				api.GET('/api/v1/me').then(ok)
			]);
			users = list;
			me = who.user?.id ?? null;
		} catch (e) {
			failure = e;
		}
	}
	load();

	async function run(action: () => Promise<unknown>) {
		busy = true;
		failure = null;
		try {
			await action();
			await load();
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}

	/** The full link to open on the invited person's device. */
	const link = (i: Invite) =>
		new URL(resolve('/invite/[code]', { code: i.code }), document.baseURI).href;

	const add = () =>
		run(async () => {
			const created = ok(await api.POST('/api/v1/users', { body: { name: adding?.trim() ?? '' } }));
			invites[created.user.id] = created.invite;
			adding = null;
		});

	const newInvite = (u: Schemas['User']) =>
		run(async () => {
			invites[u.id] = ok(
				await api.POST('/api/v1/users/{id}/invites', { params: { path: { id: u.id } } })
			);
		});

	/** Registers another passkey for yourself, here. */
	const addHere = (u: Schemas['User']) =>
		run(async () => {
			const i = ok(
				await api.POST('/api/v1/users/{id}/invites', { params: { path: { id: u.id } } })
			);
			await goto(resolve('/invite/[code]', { code: i.code }));
		});

	function removeUser(u: Schemas['User']) {
		const yourself = u.id === me ? ' This is you: you’ll be signed out.' : '';
		if (!confirm(`Remove ${u.name}? Their passkeys and sessions stop working.${yourself}`)) return;
		run(async () =>
			done(await api.DELETE('/api/v1/users/{id}', { params: { path: { id: u.id } } }))
		);
	}

	function removePasskey(u: Schemas['User'], p: Schemas['Passkey']) {
		if (!confirm(`Delete ${u.name}’s passkey “${p.name}”? It stops working at once.`)) return;
		run(async () =>
			done(await api.DELETE('/api/v1/passkeys/{id}', { params: { path: { id: p.id } } }))
		);
	}

	async function copy(text: string) {
		await navigator.clipboard?.writeText(text).catch(() => {});
	}
</script>

<div class="heading">
	<h1>Users</h1>
	<button type="button" disabled={!users || adding !== null} onclick={() => (adding = '')}
		>Add user</button
	>
</div>
<p class="muted intro">
	Who can sign in. Each person signs in with a passkey; an invite link adds one on a new device or
	replaces lost ones. Links work once, for 24 hours.
</p>

{#if adding !== null}
	<form
		class="card add"
		onsubmit={(e) => {
			e.preventDefault();
			add();
		}}
	>
		<label class="field grow">
			<span>Name</span>
			<!-- svelte-ignore a11y_autofocus -->
			<input bind:value={adding} maxlength="64" required autofocus />
		</label>
		<button type="submit" disabled={busy}>Add and make an invite</button>
		<button type="button" class="quiet" onclick={() => (adding = null)}>Cancel</button>
	</form>
{/if}

{#if failure}<Failure error={failure} />{/if}

{#if users}
	{#each users as u (u.id)}
		<section class="card">
			<div class="heading">
				<h2>
					{u.name}{#if u.id === me}<span class="muted"> (you)</span>{/if}
				</h2>
				<div class="actions">
					{#if u.id === me}
						<button class="link" onclick={() => addHere(u)} disabled={busy}
							>Add a passkey here</button
						>
					{/if}
					<button class="link" onclick={() => newInvite(u)} disabled={busy}>New invite link</button>
					<button class="link danger" onclick={() => removeUser(u)} disabled={busy}>Remove</button>
				</div>
			</div>
			{#if invites[u.id]}
				{@const i = invites[u.id]}
				<p class="invite">
					Open on {u.name}’s device, before {new Date(i.expires_at).toLocaleString()}:
					<code>{link(i)}</code>
					<button class="link" onclick={() => copy(link(i))}>Copy</button>
				</p>
			{/if}
			{#if u.passkeys.length}
				<table class="list">
					<thead><tr><th>Passkey</th><th>Added</th><th>Last used</th><th></th></tr></thead>
					<tbody>
						{#each u.passkeys as p (p.id)}
							<tr>
								<td>{p.name}</td>
								<td>{localDate(p.created_at)}</td>
								<td>{p.last_used_at ? localDate(p.last_used_at) : '—'}</td>
								<td class="actions">
									<button class="link danger" onclick={() => removePasskey(u, p)} disabled={busy}
										>Delete</button
									>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{:else}
				<p class="muted">No passkey yet: send them an invite link.</p>
			{/if}
		</section>
	{:else}
		<p class="muted">Nobody signs in with a passkey yet.</p>
	{/each}
{:else if !failure}
	<p class="muted">Loading…</p>
{/if}

<style>
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-4);
	}
	.intro {
		margin-bottom: var(--space-4);
	}
	.card {
		margin-bottom: var(--space-4);
	}
	.add {
		display: flex;
		align-items: end;
		flex-wrap: wrap;
		gap: var(--space-3);
	}
	.actions {
		display: flex;
		gap: var(--space-3);
		white-space: nowrap;
	}
	td.actions {
		justify-content: end;
	}
	.invite {
		overflow-wrap: anywhere;
	}
</style>
