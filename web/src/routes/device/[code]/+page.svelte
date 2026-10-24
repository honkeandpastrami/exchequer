<script lang="ts">
	import { page } from '$app/state';
	import { api, done, ok } from '#lib/api/client.ts';
	import Failure from '#lib/components/Failure.svelte';

	// The CLI's `exchequer login` sends people here with the code it shows.
	const code = $derived(page.params.code ?? '');
	let busy = $state(false);
	let approved = $state(false);
	let failure: unknown = $state(null);

	const pending = $derived(
		api.GET('/api/v1/devices/{code}', { params: { path: { code } } }).then(ok)
	);

	async function approve() {
		busy = true;
		failure = null;
		try {
			done(await api.POST('/api/v1/devices/{code}/approve', { params: { path: { code } } }));
			approved = true;
		} catch (e) {
			failure = e;
		} finally {
			busy = false;
		}
	}
</script>

<h1>Sign in the command line</h1>
{#await pending}
	<p class="muted">Loading…</p>
{:then d}
	{#if approved}
		<p>Done: <strong>{d.client}</strong> is signed in as you. You can close this page.</p>
	{:else}
		<p>
			<strong>{d.client}</strong> is asking to sign in as you with the code
			<code>{code.toUpperCase()}</code>, with a token that works for {d.ttl_days} days. Approve it only
			if that's the code your terminal shows.
		</p>
		<button type="button" onclick={approve} disabled={busy}>Approve</button>
	{/if}
	{#if failure}<Failure error={failure} />{/if}
{:catch}
	<Failure
		error={new Error(
			'No sign-in is waiting under this code; it may have expired. Run exchequer login again.'
		)}
	/>
{/await}
