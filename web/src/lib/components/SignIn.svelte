<script lang="ts">
	import { api, ApiError, ok } from '../api/client';
	import { signInWithPasskey } from '../passkeys';
	import { signIn } from '../session.svelte';

	let token = $state('');
	let busy = $state(false);
	let message = $state('');
	/** Whether the server offers passkeys; unknown until it answers. */
	let passkeys: boolean | null = $state(null);

	api
		.GET('/api/v1/auth')
		.then((r) => (passkeys = ok(r).passkeys))
		.catch(() => (passkeys = false));

	async function withPasskey() {
		busy = true;
		message = '';
		try {
			await signInWithPasskey();
		} catch (e) {
			message =
				e instanceof ApiError && e.status === 401
					? 'That passkey was not accepted.'
					: e instanceof Error
						? e.message
						: String(e);
		} finally {
			busy = false;
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		message = '';
		const candidate = token.trim();
		try {
			// Tried before it's kept, so a wrong token never reaches the app.
			ok(
				await api.GET('/api/v1/parties', {
					params: { query: { limit: 1 } },
					headers: { Authorization: `Bearer ${candidate}` }
				})
			);
			signIn(candidate);
		} catch (e) {
			message =
				e instanceof ApiError && e.status === 401
					? 'That token was not accepted.'
					: 'The server could not be reached.';
		} finally {
			busy = false;
			token = '';
		}
	}
</script>

<div class="sign-in">
	<h1>Exchequer</h1>
	{#if passkeys}
		<button type="button" onclick={withPasskey} disabled={busy}>Sign in with a passkey</button>
	{/if}
	{#if message}<p class="message" role="alert">{message}</p>{/if}
	{#if passkeys !== null}
		<details open={!passkeys}>
			<summary class="muted">Use an API token</summary>
			<form onsubmit={submit}>
				<label for="token">API token</label>
				<input
					id="token"
					type="password"
					autocomplete="off"
					placeholder="exq_…"
					bind:value={token}
					required
				/>
				<button type="submit" disabled={busy}>Sign in</button>
			</form>
		</details>
	{/if}
	<p class="muted hint">Kept in this browser until you sign out.</p>
</div>

<style>
	.sign-in {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		max-width: 24rem;
		margin: var(--space-6) auto;
		padding: var(--space-5);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		margin-top: var(--space-3);
	}
	summary {
		cursor: pointer;
		font-size: var(--text-sm);
	}
	label {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.message {
		margin: 0;
		color: var(--color-negative);
	}
	.hint {
		margin: 0;
		font-size: var(--text-xs);
	}
</style>
