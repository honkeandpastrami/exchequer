<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { ApiError } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';
	import { register } from '#lib/passkeys.ts';
	import { session } from '#lib/session.svelte.ts';

	let busy = $state(false);
	let failure: unknown = $state(null);

	async function create() {
		busy = true;
		failure = null;
		try {
			await register(page.params.code ?? '');
			await goto(resolve('/'), { replace: true });
		} catch (e) {
			failure =
				e instanceof ApiError && e.status === 404
					? new Error('This invite was used, expired or is mistyped. Ask for a new one.')
					: e;
		} finally {
			busy = false;
		}
	}
</script>

<div class="invite">
	<h1>Exchequer</h1>
	<p>
		This invite adds a passkey on this device: Face ID, Touch ID, Windows Hello or a security key.
		There's no password.
	</p>
	{#if session.token}
		<p class="muted">
			You'll be signed in as the person it was made for, which may not be who's signed in now.
		</p>
	{/if}
	<button type="button" onclick={create} disabled={busy}>Create a passkey</button>
	{#if failure}<Failure error={failure} />{/if}
	<p class="muted hint">The link works once, for 24 hours.</p>
</div>

<style>
	.invite {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		max-width: 28rem;
		margin: var(--space-6) auto;
		padding: var(--space-5);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	p {
		margin: 0;
	}
	.hint {
		font-size: var(--text-xs);
	}
</style>
