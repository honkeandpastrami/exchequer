<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { localDate } from '#lib/dates.ts';
	import { signOut } from '#lib/session.svelte.ts';

	type Invite = Schemas['Invite'];
	type Token = Schemas['Token'];

	let users: Schemas['User'][] | null = $state(null);
	let tokens: Token[] | null = $state(null);
	let me: string | null = $state(null);
	/** This browser's token. */
	let mine: string | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let adding: string | null = $state(null);
	/** The newest invite link per user, shown until the page is left. */
	let invites: Record<string, Invite> = $state({});
	/** A new token being made: its name and time to live. */
	let making: { name: string; days: number } | null = $state(null);
	/** The token just made, shown once until the page is left. */
	let made: Schemas['CreatedToken'] | null = $state(null);
	let everything = $state(false);

	const KINDS: Record<Token['kind'], string> = {
		session: 'Browser',
		cli: 'CLI',
		script: 'Script'
	};

	const working = (t: Token) => !t.revoked_at && new Date(t.expires_at) > new Date();
	const shown = $derived.by(() => tokens?.filter((t) => everything || working(t)) ?? []);
	const owner = (t: Token) => users?.find((u) => u.id === t.user_id)?.name ?? '—';

	async function load() {
		try {
			const [list, who, all] = await Promise.all([
				api.GET('/api/v1/users').then(ok),
				api.GET('/api/v1/me').then(ok),
				api.GET('/api/v1/tokens').then(ok)
			]);
			users = list;
			me = who.user?.id ?? null;
			mine = who.token?.id ?? null;
			tokens = all;
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

	const makeToken = () =>
		run(async () => {
			const t = making ?? { name: '', days: 0 };
			made = ok(
				await api.POST('/api/v1/tokens', { body: { name: t.name.trim(), ttl_days: t.days } })
			);
			making = null;
		});

	function revoke(t: Token) {
		const yours = t.id === mine ? ' It’s this browser’s: you’ll be signed out.' : '';
		if (!confirm(`Revoke “${t.name}”? It stops working at once.${yours}`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/tokens/{id}', { params: { path: { id: t.id } } }));
			if (t.id === mine) signOut();
		});
	}

	function revokeAll() {
		if (
			!confirm(
				'Revoke every token? Everyone is signed out, you included, and every CLI and script stops working. Passkeys still sign in.'
			)
		)
			return;
		run(async () => {
			ok(await api.DELETE('/api/v1/tokens'));
			signOut();
		});
	}

	async function copy(text: string) {
		await navigator.clipboard?.writeText(text).catch(() => {});
	}
</script>

<h1>Security</h1>
<p class="muted intro">
	Who can sign in, and every token that works for them. A token is how the server knows who is
	asking: each browser signed in has one, so does each CLI after <code>exchequer login</code>, and
	so does each script. Every token expires; revoking one stops it at once.
</p>

<div class="heading">
	<h2>Users</h2>
	<button type="button" disabled={!users || adding !== null} onclick={() => (adding = '')}
		>Add user</button
	>
</div>
<p class="muted intro">
	Each person signs in with a passkey; an invite link adds one on a new device or replaces lost
	ones. Links work once, for 24 hours.
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
				<h3>
					{u.name}{#if u.id === me}<span class="muted"> (you)</span>{/if}
				</h3>
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

<div class="heading">
	<h2>Tokens</h2>
	<button
		type="button"
		disabled={!tokens || making !== null}
		onclick={() => (making = { name: '', days: 90 })}>New token for a script</button
	>
</div>
<p class="muted intro">
	A token for a script is yours, and works for the days you choose, at most 365. The CLI makes its
	own with <code>exchequer login</code>; a browser, when you sign in.
</p>

{#if making !== null}
	<form
		class="card add"
		onsubmit={(e) => {
			e.preventDefault();
			makeToken();
		}}
	>
		<label class="field grow">
			<span>What uses it</span>
			<!-- svelte-ignore a11y_autofocus -->
			<input bind:value={making.name} maxlength="64" required autofocus />
		</label>
		<label class="field">
			<span>Days it works</span>
			<input type="number" bind:value={making.days} min="1" max="365" required />
		</label>
		<button type="submit" disabled={busy}>Make it</button>
		<button type="button" class="quiet" onclick={() => (making = null)}>Cancel</button>
	</form>
{/if}

{#if made}
	<p class="card invite">
		<strong>{made.name}</strong>, until {new Date(made.expires_at).toLocaleString()}. Copy it now:
		it’s shown only once.
		<code>{made.token}</code>
		<button class="link" onclick={() => made && copy(made.token)}>Copy</button>
	</p>
{/if}

{#if tokens}
	<label class="toggle"
		><input type="checkbox" bind:checked={everything} /> Show revoked and expired</label
	>
	{#if shown.length}
		<table class="list">
			<thead>
				<tr
					><th>Token</th><th>User</th><th>Kind</th><th>Created</th><th>Last used</th><th>Expires</th
					><th></th></tr
				>
			</thead>
			<tbody>
				{#each shown as t (t.id)}
					<tr class:muted={!working(t)}>
						<td
							>{t.name}{#if t.id === mine}<span class="muted"> (this browser)</span>{/if}</td
						>
						<td>{owner(t)}</td>
						<td>{KINDS[t.kind]}</td>
						<td>{localDate(t.created_at)}</td>
						<td>{t.last_used_at ? localDate(t.last_used_at) : '—'}</td>
						<td>{localDate(t.expires_at)}</td>
						<td class="actions">
							{#if t.revoked_at}
								revoked {localDate(t.revoked_at)}
							{:else if working(t)}
								<button class="link danger" onclick={() => revoke(t)} disabled={busy}>Revoke</button
								>
							{:else}
								expired
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{:else}
		<p class="muted">No tokens.</p>
	{/if}

	<section class="card kill">
		<h3>Revoke every token</h3>
		<p class="muted">
			If a token may have leaked. Everyone is signed out, you included, and every CLI and script
			stops working. Passkeys still sign in; the CLI signs in again with
			<code>exchequer login</code>, and scripts need new tokens.
		</p>
		<button onclick={revokeAll} disabled={busy}>Revoke all tokens</button>
	</section>
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
	h2 {
		margin-top: var(--space-6);
	}
	.toggle {
		display: block;
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
	}
	.kill {
		margin-top: var(--space-6);
	}
	.kill button {
		border-color: var(--color-negative);
		background: var(--color-negative);
	}
</style>
