<script lang="ts">
	// Who can sign in, and every token (design §10). Rows and forms as in
	// docs/ux.md §10: Add or Edit opens the row's form; what acts on a user or
	// a token is in its form.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import NoValue from '#lib/components/NoValue.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import { localDate } from '#lib/dates.ts';
	import { signOut } from '#lib/session.svelte.ts';

	type User = Schemas['User'];
	type Token = Schemas['Token'];
	type Invite = Schemas['Invite'];
	/** A user's form: a new user (`id` null) or one's, with the invite just made. */
	type UserForm = {
		id: string | null;
		etag: string;
		name: string;
		replace: boolean;
		invite: Invite | null;
	};
	/** A token's form: a new token (`id` null) or one's, with its secret if just made. */
	type TokenForm = { id: string | null; name: string; days: number; secret: string | null };

	const KINDS: Record<Token['kind'], string> = { session: 'Session', cli: 'CLI', script: 'Script' };

	let users: User[] | null = $state(null);
	let tokens: Token[] | null = $state(null);
	let me: string | null = $state(null);
	/** This browser's token. */
	let mine: string | null = $state(null);
	let loadFailure: unknown = $state(null);
	let userForm: UserForm | null = $state(null);
	let tokenForm: TokenForm | null = $state(null);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let everything = $state(false);

	const working = (t: Token) => !t.revoked_at && new Date(t.expires_at) > new Date();
	const shown = $derived.by(() => tokens?.filter((t) => everything || working(t)) ?? []);
	const owner = (t: Token) => users?.find((u) => u.id === t.user_id)?.name ?? null;
	const tokensOf = (u: User) => tokens?.filter((t) => t.user_id === u.id && working(t)) ?? [];

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
			loadFailure = null;
		} catch (e) {
			loadFailure = e;
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

	function openUser(f: UserForm | null) {
		failure = null;
		tokenForm = null;
		userForm = f;
	}

	function openToken(f: TokenForm | null) {
		failure = null;
		userForm = null;
		tokenForm = f;
	}

	/** The user as stored now, and the ETag to rename them with. */
	async function readUser(id: string) {
		const r = await api.GET('/api/v1/users/{id}', { params: { path: { id } } });
		return { user: ok(r), etag: r.response.headers.get('etag') ?? '' };
	}

	async function editUser(u: User, again = false) {
		if (!again && userForm?.id === u.id) return openUser(null);
		try {
			const { user, etag } = await readUser(u.id);
			openUser({ id: u.id, etag, name: user.name, replace: false, invite: null });
		} catch (e) {
			failure = e;
		}
	}

	const editToken = (t: Token) =>
		openToken(tokenForm?.id === t.id ? null : { id: t.id, name: t.name, days: 0, secret: null });

	async function invite(name: string, replace: boolean) {
		return ok(await api.POST('/api/v1/invites', { body: { name, replace } }));
	}

	/** Renames the user; or adds one, and their form stays open showing their
	 * first invite. */
	const saveUser = () =>
		run(async () => {
			const f = userForm;
			if (!f) return;
			const name = f.name.trim();
			if (f.id) {
				if (name !== users?.find((u) => u.id === f.id)?.name) {
					ok(
						await api.PATCH('/api/v1/users/{id}', {
							params: { path: { id: f.id }, header: { 'If-Match': f.etag } },
							body: { name }
						})
					);
				}
				userForm = null;
				return;
			}
			const made = await invite(name, false);
			const { etag } = await readUser(made.user.id);
			userForm = {
				id: made.user.id,
				etag,
				name: made.user.name,
				replace: false,
				invite: made.invite
			};
		});

	function inviteUser(u: User, f: UserForm) {
		if (f.replace) {
			const n = u.passkeys.length;
			const t = tokensOf(u).length;
			if (
				!confirm(
					`When the link is used, ${u.name}’s ${n} passkeys are deleted and their ${t} tokens revoked.`
				)
			)
				return;
		}
		run(async () => {
			f.invite = (await invite(u.name, f.replace)).invite;
		});
	}

	/** Registers another passkey for yourself, here. */
	const addHere = (u: User) =>
		run(async () => {
			const made = await invite(u.name, false);
			await goto(resolve('/invite/[code]', { code: made.invite.code }));
		});

	function removeUser(u: User) {
		const yourself = u.id === me ? ' This is you: you’ll be signed out.' : '';
		if (!confirm(`Delete ${u.name}? Their passkeys and tokens stop working.${yourself}`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/users/{id}', { params: { path: { id: u.id } } }));
			userForm = null;
			if (u.id === me) signOut();
		});
	}

	function removePasskey(u: User, p: Schemas['Passkey']) {
		if (!confirm(`Delete ${u.name}’s passkey “${p.name}”? It stops working at once.`)) return;
		run(async () =>
			done(await api.DELETE('/api/v1/passkeys/{id}', { params: { path: { id: p.id } } }))
		);
	}

	/** Makes the token; its form stays open, showing it once. */
	const saveToken = () =>
		run(async () => {
			const f = tokenForm;
			if (!f) return;
			if (f.id) {
				tokenForm = null;
				return;
			}
			const made = ok(
				await api.POST('/api/v1/tokens', { body: { name: f.name.trim(), ttl_days: f.days } })
			);
			tokenForm = { id: made.id, name: made.name, days: f.days, secret: made.token };
		});

	function revoke(t: Token) {
		const yours = t.id === mine ? ' It’s this browser’s: you’ll be signed out.' : '';
		if (!confirm(`Revoke “${t.name}”? It stops working at once.${yours}`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/tokens/{id}', { params: { path: { id: t.id } } }));
			tokenForm = null;
			if (t.id === mine) signOut();
		});
	}

	function revokeAll() {
		if (
			!confirm(
				'Revoke every token? Everyone is signed out, you included, and every CLI and script stops working.'
			)
		)
			return;
		run(async () => {
			ok(await api.DELETE('/api/v1/tokens'));
			signOut();
		});
	}

	/** The full link to open on the invited person's device. */
	const link = (i: Invite) =>
		new URL(resolve('/invite/[code]', { code: i.code }), document.baseURI).href;

	async function copy(text: string) {
		await navigator.clipboard?.writeText(text).catch(() => {});
	}
</script>

{#snippet userFields(f: UserForm, u?: User)}
	<RowForm
		label={u ? `Edit ${u.name}` : 'Add user'}
		{busy}
		{failure}
		onsave={saveUser}
		oncancel={() => openUser(null)}
		onreload={u ? () => editUser(u, true) : undefined}
	>
		<div class="fields">
			<label class="field grow">
				<span>Name</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input bind:value={f.name} maxlength="64" required autofocus={!u} />
			</label>
			{#if u}
				<label class="check"
					><input type="checkbox" bind:checked={f.replace} /> Invite replaces their passkeys</label
				>
			{/if}
		</div>
		{#if f.invite}
			<p class="secret">
				Open once on {u?.name ?? f.name}’s device, before {new Date(
					f.invite.expires_at
				).toLocaleString()}:
				<code>{link(f.invite)}</code>
				<button type="button" class="link" onclick={() => f.invite && copy(link(f.invite))}
					>Copy</button
				>
			</p>
		{/if}
		{#if u}
			{#if u.passkeys.length}
				<table class="passkeys">
					<thead><tr><th>Passkey</th><th>Added</th><th>Last used</th><th></th></tr></thead>
					<tbody>
						{#each u.passkeys as p (p.id)}
							<tr>
								<td>{p.name}</td>
								<td>{localDate(p.created_at)}</td>
								<td
									>{#if p.last_used_at}{localDate(p.last_used_at)}{:else}<NoValue
											kind="none"
										/>{/if}</td
								>
								<td class="actions">
									<button
										type="button"
										class="link danger"
										onclick={() => removePasskey(u, p)}
										disabled={busy}>Delete</button
									>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
				<p class="muted hint">A passkey is deleted at once.</p>
			{:else}
				<p class="muted hint">No passkey yet: invite them.</p>
			{/if}
		{/if}
		{#snippet actions()}
			{#if u}
				{#if u.id === me}
					<button type="button" class="quiet" onclick={() => addHere(u)} disabled={busy}
						>Add a passkey here</button
					>
				{/if}
				<button type="button" class="quiet" onclick={() => inviteUser(u, f)} disabled={busy}
					>Invite</button
				>
				<button type="button" class="quiet danger" onclick={() => removeUser(u)} disabled={busy}
					>Delete</button
				>
			{/if}
		{/snippet}
		{#snippet facts()}
			{#if u}<span>Added {localDate(u.created_at)}</span><span>ID {u.id}</span>{/if}
		{/snippet}
	</RowForm>
{/snippet}

{#snippet tokenFields(f: TokenForm, t?: Token)}
	<RowForm
		label={t ? `Edit ${t.name}` : 'Add token'}
		{busy}
		{failure}
		onsave={saveToken}
		oncancel={() => openToken(null)}
	>
		<div class="fields">
			<label class="field grow">
				<span>Name</span>
				<!-- A token's name and time to live are set when it's made. -->
				<!-- svelte-ignore a11y_autofocus -->
				<input bind:value={f.name} maxlength="64" required disabled={!!t} autofocus={!t} />
			</label>
			{#if !t}
				<label class="field">
					<span>Days it works</span>
					<input type="number" bind:value={f.days} min="1" max="365" required />
				</label>
			{/if}
		</div>
		{#if f.secret}
			<p class="secret">
				Shown only now; copy it: <code>{f.secret}</code>
				<button type="button" class="link" onclick={() => f.secret && copy(f.secret)}>Copy</button>
			</p>
		{/if}
		{#snippet actions()}
			{#if t && working(t)}
				<button type="button" class="quiet danger" onclick={() => revoke(t)} disabled={busy}
					>Revoke</button
				>
			{/if}
		{/snippet}
		{#snippet facts()}
			{#if t}
				<span>{KINDS[t.kind]} of {owner(t) ?? 'nobody'}</span>
				<span>Created {localDate(t.created_at)}</span>
				<span>Expires {localDate(t.expires_at)}</span>
				{#if t.revoked_at}<span>Revoked {localDate(t.revoked_at)}</span>{/if}
				<span>ID {t.id}</span>
			{/if}
		{/snippet}
	</RowForm>
{/snippet}

<h1>Security</h1>

{#if users && tokens}
	<div class="heading">
		<h2>Users</h2>
		<button
			type="button"
			disabled={userForm?.id === null}
			onclick={() => openUser({ id: null, etag: '', name: '', replace: false, invite: null })}
			>Add user</button
		>
	</div>
	<p class="muted intro">
		Each signs in with a passkey; an invite link, once, for 3 hours, adds one.
	</p>
	<div class="card">
		<table class="list">
			<thead><tr><th>Name</th><th>Passkeys</th><th>Added</th><th></th></tr></thead>
			<tbody>
				{#if userForm && userForm.id === null}
					<tr class="open"><td colspan="4">{@render userFields(userForm)}</td></tr>
				{/if}
				{#each users as u (u.id)}
					<tr>
						<td
							>{u.name}{#if u.id === me}<span class="muted"> (you)</span>{/if}</td
						>
						<td>{u.passkeys.length}</td>
						<td>{localDate(u.created_at)}</td>
						<td class="actions">
							<button class="link" aria-expanded={userForm?.id === u.id} onclick={() => editUser(u)}
								>Edit</button
							>
						</td>
					</tr>
					{#if userForm && userForm.id === u.id}
						<tr class="open"><td colspan="4">{@render userFields(userForm, u)}</td></tr>
					{/if}
				{/each}
			</tbody>
		</table>
	</div>

	<div class="heading">
		<h2>Tokens</h2>
		<button
			type="button"
			disabled={tokenForm?.id === null}
			onclick={() => openToken({ id: null, name: '', days: 90, secret: null })}>Add token</button
		>
	</div>
	<p class="muted intro">Sessions, CLIs and scripts. Each expires; a revoked one stops at once.</p>
	<label class="toggle"
		><input type="checkbox" bind:checked={everything} /> Show revoked and expired</label
	>
	<div class="card">
		<table class="list">
			<thead>
				<tr>
					<th>Token</th><th>User</th><th>Kind</th><th>Created</th><th>Last used</th><th>Expires</th
					><th></th>
				</tr>
			</thead>
			<tbody>
				{#if tokenForm && tokenForm.id === null}
					<tr class="open"><td colspan="7">{@render tokenFields(tokenForm)}</td></tr>
				{/if}
				{#each shown as t (t.id)}
					<tr class:muted={!working(t)}>
						<td
							>{t.name}{#if t.id === mine}<span class="muted"> (this browser)</span>{/if}</td
						>
						<td
							>{#if owner(t)}{owner(t)}{:else}<NoValue kind="none" />{/if}</td
						>
						<td>{KINDS[t.kind]}</td>
						<td>{localDate(t.created_at)}</td>
						<td
							>{#if t.last_used_at}{localDate(t.last_used_at)}{:else}<NoValue
									kind="none"
								/>{/if}</td
						>
						<td>{t.revoked_at ? 'revoked' : localDate(t.expires_at)}</td>
						<td class="actions">
							<button
								class="link"
								aria-expanded={tokenForm?.id === t.id}
								onclick={() => editToken(t)}>Edit</button
							>
						</td>
					</tr>
					{#if tokenForm && tokenForm.id === t.id}
						<tr class="open"><td colspan="7">{@render tokenFields(tokenForm, t)}</td></tr>
					{/if}
				{/each}
			</tbody>
		</table>
	</div>

	<div class="heading">
		<h2>Every token</h2>
		<button type="button" class="kill" onclick={revokeAll} disabled={busy}>Revoke all tokens</button
		>
	</div>
	<p class="muted intro">If one leaked: everyone is signed out; passkeys still sign in.</p>
	{#if failure && !userForm && !tokenForm}<Failure error={failure} />{/if}
{:else if loadFailure}
	<Failure error={loadFailure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-4);
		margin-top: var(--space-5);
	}
	.intro {
		margin-bottom: var(--space-3);
	}
	.card {
		margin-bottom: var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
		overflow-x: auto;
	}
	.list {
		min-width: 36rem;
	}
	.open td {
		border-top: none;
		padding-top: 0;
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	.toggle {
		display: block;
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
	}
	.secret {
		overflow-wrap: anywhere;
	}
	.passkeys {
		margin-bottom: var(--space-2);
	}
	.hint {
		font-size: var(--text-sm);
	}
	.kill {
		border-color: var(--color-negative);
		background: var(--color-negative);
	}
</style>
