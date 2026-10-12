<script lang="ts">
	import '#lib/theme/tokens.css';
	import '#lib/theme/base.css';
	import favicon from '#lib/assets/favicon.svg';
	import Shell from '#lib/components/Shell.svelte';
	import SignIn from '#lib/components/SignIn.svelte';
	import { session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Exchequer</title>
	<link rel="icon" href={favicon} />
</svelte:head>

{#if session.token}
	<Shell>{@render children()}</Shell>
{:else}
	<SignIn />
{/if}
