<script lang="ts">
	import { api, ApiError, ok } from '../api/client';
	import { signIn } from '../session.svelte';

	let token = $state('');
	let busy = $state(false);
	let message = $state('');

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

<form class="sign-in" onsubmit={submit}>
	<h1>Exchequer</h1>
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
	{#if message}<p class="message" role="alert">{message}</p>{/if}
	<p class="muted hint">Kept in this browser until you sign out.</p>
</form>

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
