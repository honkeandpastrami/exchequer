<script lang="ts">
	// Who can sign in and how (design §10): users, their passkeys, the
	// sessions signed in, and scripts' tokens. Rows and forms as in
	// docs/ux.md §10: Add or Edit opens the row's form; what acts on an item
	// is in its form.
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, done, ok, type Schemas } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import NoValue from '#lib/components/NoValue.svelte';
	import RowForm from '#lib/components/RowForm.svelte';
	import { localDate } from '#lib/dates.ts';
	import { signOut } from '#lib/session.svelte.ts';

	type User = Schemas['User'];
	type Passkey = Schemas['Passkey'] & { user: User };
	type Session = Schemas['Session'];
	type Token = Schemas['Token'];
	type Invite = Schemas['Invite'];
	/** A user's form: a new user (`id` null) or one's, with the invite just made. */
	type UserForm = { id: string | null; etag: string; name: string; invite: Invite | null };
	/** A token's form: a new token (`id` null) or one's, with its secret if just made. */
	type TokenForm = { id: string | null; name: string; days: number; secret: string | null };
	/** Which form is open: one at a time. */
	type Open =
		| { what: 'user'; form: UserForm }
		| { what: 'passkey'; id: string }
		| { what: 'session'; id: string }
		| { what: 'token'; form: TokenForm }
		| null;

	const KINDS: Record<Session['kind'], string> = {
		browser: 'Browser',
		cli: 'CLI',
		script: 'Script'
	};

	let users: User[] | null = $state(null);
	let sessions: Session[] | null = $state(null);
	let tokens: Token[] | null = $state(null);
	let me: string | null = $state(null);
	/** This browser's session. */
	let mine: string | null = $state(null);
	let loadFailure: unknown = $state(null);
	let open = $state<Open>(null);
	let failure: unknown = $state(null);
	let busy = $state(false);
	let everything = $state(false);

	const passkeys = $derived.by((): Passkey[] =>
		(users ?? []).flatMap((u) => u.passkeys.map((p) => ({ ...p, user: u })))
	);
	const working = (t: Token) => !t.revoked_at && new Date(t.expires_at) > new Date();
	const shown = $derived.by(() => tokens?.filter((t) => everything || working(t)) ?? []);
	const nameOf = (id: string | null | undefined) => users?.find((u) => u.id === id)?.name ?? null;
	const userForm = $derived(open?.what === 'user' ? open.form : null);
	const tokenForm = $derived(open?.what === 'token' ? open.form : null);

	async function load() {
		try {
			const [list, who, signedIn, made] = await Promise.all([
				api.GET('/api/v1/users').then(ok),
				api.GET('/api/v1/me').then(ok),
				api.GET('/api/v1/sessions').then(ok),
				api.GET('/api/v1/tokens').then(ok)
			]);
			users = list;
			me = who.user?.id ?? null;
			mine = who.session?.id ?? null;
			sessions = signedIn;
			tokens = made;
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

	function show(o: Open) {
		failure = null;
		open = o;
	}

	/** Opens the passkey's or session's form, or closes it if it's open. */
	function toggle(what: 'passkey' | 'session', id: string) {
		show(open?.what === what && open.id === id ? null : { what, id });
	}

	/** The user as stored now, and the ETag to rename them with. */
	async function readUser(id: string) {
		const r = await api.GET('/api/v1/users/{id}', { params: { path: { id } } });
		return { user: ok(r), etag: r.response.headers.get('etag') ?? '' };
	}

	async function editUser(u: User, again = false) {
		if (!again && userForm?.id === u.id) return show(null);
		try {
			const { user, etag } = await readUser(u.id);
			show({ what: 'user', form: { id: u.id, etag, name: user.name, invite: null } });
		} catch (e) {
			failure = e;
		}
	}

	const editToken = (t: Token) =>
		show(
			tokenForm?.id === t.id
				? null
				: { what: 'token', form: { id: t.id, name: t.name, days: 0, secret: null } }
		);

	async function invite(name: string) {
		return ok(await api.POST('/api/v1/invites', { body: { name } }));
	}

	/** Renames the user; or adds one, and their form stays open showing their
	 * first invite. */
	const saveUser = () =>
		run(async () => {
			const f = userForm;
			if (!f) return;
			const name = f.name.trim();
			if (f.id) {
				if (name !== nameOf(f.id)) {
					ok(
						await api.PATCH('/api/v1/users/{id}', {
							params: { path: { id: f.id }, header: { 'If-Match': f.etag } },
							body: { name }
						})
					);
				}
				open = null;
				return;
			}
			const made = await invite(name);
			const { etag } = await readUser(made.user.id);
			open = {
				what: 'user',
				form: { id: made.user.id, etag, name: made.user.name, invite: made.invite }
			};
		});

	const inviteUser = (u: User, f: UserForm) =>
		run(async () => {
			f.invite = (await invite(u.name)).invite;
		});

	/** Registers another passkey for yourself, here. */
	const addHere = (u: User) =>
		run(async () => {
			const made = await invite(u.name);
			await goto(resolve('/invite/[code]', { code: made.invite.code }));
		});

	function removeUser(u: User) {
		const yourself = u.id === me ? ' This is you: you’ll be signed out.' : '';
		if (!confirm(`Delete ${u.name}? Their passkeys and tokens stop working.${yourself}`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/users/{id}', { params: { path: { id: u.id } } }));
			open = null;
			if (u.id === me) signOut();
		});
	}

	function removePasskey(p: Passkey) {
		if (!confirm(`Delete ${p.user.name}’s passkey “${p.name}”? It stops working at once.`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/passkeys/{id}', { params: { path: { id: p.id } } }));
			open = null;
		});
	}

	/** Who a session is and what it is, for a confirmation. */
	const described = (s: Session) =>
		[nameOf(s.user_id), KINDS[s.kind], s.client, s.host].filter(Boolean).join(' · ');

	function endSession(s: Session) {
		const others = sessions?.filter((o) => o.token_id === s.token_id && o.id !== s.id) ?? [];
		const also = others.length
			? ` Its token is shared, so these end too: ${others.map(described).join('; ')}.`
			: '';
		const yours = s.id === mine ? ' It’s this browser: you’ll be signed out.' : '';
		if (!confirm(`Sign out ${described(s)}?${also}${yours}`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/sessions/{id}', { params: { path: { id: s.id } } }));
			open = null;
			if (s.id === mine) signOut();
		});
	}

	/** Makes the token; its form stays open, showing it once. */
	const saveToken = () =>
		run(async () => {
			const f = tokenForm;
			if (!f) return;
			if (f.id) {
				open = null;
				return;
			}
			const made = ok(
				await api.POST('/api/v1/tokens', { body: { name: f.name.trim(), ttl_days: f.days } })
			);
			open = {
				what: 'token',
				form: { id: made.id, name: made.name, days: f.days, secret: made.token }
			};
		});

	function revoke(t: Token) {
		const n = sessions?.filter((s) => s.token_id === t.id).length ?? 0;
		const ending = n ? ` Its ${n} sessions end.` : '';
		if (!confirm(`Revoke “${t.name}”? It stops working at once.${ending}`)) return;
		run(async () => {
			done(await api.DELETE('/api/v1/tokens/{id}', { params: { path: { id: t.id } } }));
			open = null;
		});
	}

	function revokeAll() {
		if (
			!confirm(
				'Sign everyone out and revoke all tokens? Every browser, CLI and script stops working, this browser too. Passkeys still sign in.'
			)
		)
			return;
		run(async () => {
			ok(await api.POST('/api/v1/revoke-all'));
			signOut();
		});
	}

	/** The full link to open on the invited person's device. */
	const link = (i: Invite) =>
		new URL(resolve('/invite/[code]', { code: i.code }), document.baseURI).href;

	async function copy(text: string) {
		await navigator.clipboard?.writeText(text).catch(() => {});
	}

	const time = (t: string) => new Date(t).toLocaleString();
</script>

{#snippet dateOrNone(t: string | null | undefined)}
	{#if t}{localDate(t)}{:else}<NoValue kind="none" />{/if}
{/snippet}

{#snippet userFields(f: UserForm, u?: User)}
	<RowForm
		label={u ? `Edit ${u.name}` : 'Add user'}
		{busy}
		{failure}
		onsave={saveUser}
		oncancel={() => show(null)}
		onreload={u ? () => editUser(u, true) : undefined}
	>
		<div class="fields">
			<label class="field grow">
				<span>Name</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input bind:value={f.name} maxlength="64" required autofocus={!u} />
			</label>
		</div>
		{#if f.invite}
			<p class="secret">
				<code>{link(f.invite)}</code>
				<button type="button" class="link" onclick={() => f.invite && copy(link(f.invite))}
					>Copy</button
				>
				<span class="muted">Works once, until {time(f.invite.expires_at)}.</span>
			</p>
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

{#snippet passkeyFields(p: Passkey)}
	<RowForm
		label={`Edit ${p.name}`}
		{busy}
		{failure}
		onsave={() => show(null)}
		oncancel={() => show(null)}
	>
		{#snippet actions()}
			<button type="button" class="quiet danger" onclick={() => removePasskey(p)} disabled={busy}
				>Delete</button
			>
		{/snippet}
		{#snippet facts()}
			<span>ID {p.id}</span>
		{/snippet}
	</RowForm>
{/snippet}

{#snippet sessionFields(s: Session)}
	<RowForm
		label={`Edit session of ${nameOf(s.user_id) ?? 'nobody'}`}
		{busy}
		{failure}
		onsave={() => show(null)}
		oncancel={() => show(null)}
	>
		{#snippet actions()}
			<button type="button" class="quiet danger" onclick={() => endSession(s)} disabled={busy}
				>Sign out</button
			>
		{/snippet}
		{#snippet facts()}
			<span>{KINDS[s.kind]}</span>
			<span
				>IP {#if s.ip}{s.ip}{:else}<NoValue kind="unknown" />{/if}</span
			>
			<span>ID {s.id}</span>
		{/snippet}
	</RowForm>
{/snippet}

{#snippet tokenFields(f: TokenForm, t?: Token)}
	<RowForm
		label={t ? `Edit ${t.name}` : 'Add token'}
		{busy}
		{failure}
		onsave={saveToken}
		oncancel={() => show(null)}
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
				{#if t.revoked_at}<span>Revoked {localDate(t.revoked_at)}</span>{/if}
				<span>ID {t.id}</span>
			{/if}
		{/snippet}
	</RowForm>
{/snippet}

<h1>Security</h1>

{#if users && sessions && tokens}
	<div class="heading">
		<h2>Users</h2>
		<button
			type="button"
			disabled={userForm?.id === null}
			onclick={() => show({ what: 'user', form: { id: null, etag: '', name: '', invite: null } })}
			>Add user</button
		>
	</div>
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
							>{u.name}{#if u.id === me}<span class="muted aside">(you)</span>{/if}</td
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

	<h2>Passkeys</h2>
	<div class="card">
		<table class="list">
			<thead>
				<tr><th>Passkey</th><th>User</th><th>Added</th><th>Last used</th><th></th></tr>
			</thead>
			<tbody>
				{#each passkeys as p (p.id)}
					<tr>
						<td>{p.name}</td>
						<td>{p.user.name}</td>
						<td>{localDate(p.created_at)}</td>
						<td>{@render dateOrNone(p.last_used_at)}</td>
						<td class="actions">
							<button
								class="link"
								aria-expanded={open?.what === 'passkey' && open.id === p.id}
								onclick={() => toggle('passkey', p.id)}>Edit</button
							>
						</td>
					</tr>
					{#if open?.what === 'passkey' && open.id === p.id}
						<tr class="open"><td colspan="5">{@render passkeyFields(p)}</td></tr>
					{/if}
				{:else}
					<tr><td colspan="5"><NoValue kind="none" /></td></tr>
				{/each}
			</tbody>
		</table>
	</div>

	<h2>Sessions</h2>
	<div class="card">
		<table class="list wide">
			<thead>
				<tr>
					<th>User</th><th>Signed in with</th><th>Started</th><th>Last used</th>
					<th>Last used from</th><th>Expires</th><th></th>
				</tr>
			</thead>
			<tbody>
				{#each sessions as s (s.id)}
					<tr>
						<td
							>{nameOf(s.user_id)}{#if s.id === mine}<span class="muted aside">(this browser)</span
								>{/if}</td
						>
						<td>
							{#if s.signed_in_with}{s.signed_in_with}{:else}<NoValue kind="unknown" />{/if}
						</td>
						<td>{localDate(s.started_at)}</td>
						<td>{localDate(s.last_used_at)}</td>
						<td>
							{#if s.client || s.host}
								{[s.client, s.host].filter(Boolean).join(' · ')}
							{:else}
								<NoValue kind="unknown" />
							{/if}
						</td>
						<td>{localDate(s.expires_at)}</td>
						<td class="actions">
							<button
								class="link"
								aria-expanded={open?.what === 'session' && open.id === s.id}
								onclick={() => toggle('session', s.id)}>Edit</button
							>
						</td>
					</tr>
					{#if open?.what === 'session' && open.id === s.id}
						<tr class="open"><td colspan="7">{@render sessionFields(s)}</td></tr>
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
			onclick={() => show({ what: 'token', form: { id: null, name: '', days: 90, secret: null } })}
			>Add token</button
		>
	</div>
	<label class="toggle"
		><input type="checkbox" bind:checked={everything} /> Show revoked and expired</label
	>
	<div class="card">
		<table class="list">
			<thead>
				<tr>
					<th>Token</th><th>User</th><th>Created</th><th>Last used</th><th>Expires</th><th></th>
				</tr>
			</thead>
			<tbody>
				{#if tokenForm && tokenForm.id === null}
					<tr class="open"><td colspan="6">{@render tokenFields(tokenForm)}</td></tr>
				{/if}
				{#each shown as t (t.id)}
					<tr class:muted={!working(t)}>
						<td>{t.name}</td>
						<td>
							{#if nameOf(t.user_id)}{nameOf(t.user_id)}{:else}<NoValue kind="none" />{/if}
						</td>
						<td>{localDate(t.created_at)}</td>
						<td>{@render dateOrNone(t.last_used_at)}</td>
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
						<tr class="open"><td colspan="6">{@render tokenFields(tokenForm, t)}</td></tr>
					{/if}
				{:else}
					{#if !tokenForm}<tr><td colspan="6"><NoValue kind="none" /></td></tr>{/if}
				{/each}
			</tbody>
		</table>
	</div>

	<p class="kill-switch">
		<button type="button" class="kill" onclick={revokeAll} disabled={busy}
			>Sign everyone out and revoke all tokens</button
		>
	</p>
	{#if failure && !open}<Failure error={failure} />{/if}
{:else if loadFailure}
	<Failure error={loadFailure} />
{:else}
	<p class="muted">Loading…</p>
{/if}

<style>
	h2 {
		margin-top: var(--space-5);
	}
	.heading {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-4);
		margin-top: var(--space-5);
	}
	.heading h2 {
		margin-top: 0;
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
	.list.wide {
		min-width: 48rem;
	}
	.open td {
		border-top: none;
		padding-top: 0;
	}
	.actions {
		text-align: right;
		white-space: nowrap;
	}
	/* "(you)", "(this browser)": apart from the name before it. */
	.aside {
		margin-left: 0.3em;
	}
	.toggle {
		display: block;
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
	}
	.secret {
		overflow-wrap: anywhere;
	}
	.kill-switch {
		margin-top: var(--space-5);
	}
	.kill {
		border-color: var(--color-negative);
		background: var(--color-negative);
	}
</style>
