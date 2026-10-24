<script lang="ts">
	import '#lib/theme/tokens.css';
	import '#lib/theme/base.css';
	import Shell from '#lib/components/Shell.svelte';
	import { page } from '$app/state';
	import SignIn from '#lib/components/SignIn.svelte';
	import { session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Exchequer</title>
</svelte:head>

{#if session.token}
	<Shell>{@render children()}</Shell>
{:else if page.route.id === '/invite/[code]'}
	<!-- An invite is how someone without a passkey gets one. -->
	{@render children()}
{:else}
	<SignIn />
{/if}
