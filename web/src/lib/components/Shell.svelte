<script lang="ts">
	import type { Snippet } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { signOut } from '../session.svelte';

	let { children }: { children: Snippet } = $props();

	const sections = [
		{ href: resolve('/'), label: 'Accounts', route: '/' },
		{ href: resolve('/transactions'), label: 'Transactions', route: '/transactions' }
	];
</script>

<header class="bar">
	<span class="brand">Exchequer</span>
	<nav>
		{#each sections as s (s.route)}
			<a href={s.href} aria-current={page.route.id === s.route ? 'page' : undefined}>{s.label}</a>
		{/each}
	</nav>
	<button class="quiet" onclick={signOut}>Sign out</button>
</header>
<main>
	{@render children()}
</main>

<style>
	.bar {
		display: flex;
		align-items: center;
		gap: var(--space-5);
		padding: var(--space-2) var(--gutter);
		background: var(--color-surface);
		border-bottom: var(--border-width) solid var(--color-border);
	}
	.brand {
		font-family: var(--font-display);
		font-size: var(--text-lg);
		font-weight: var(--weight-strong);
	}
	nav {
		display: flex;
		gap: var(--space-4);
		flex: 1;
	}
	nav a {
		text-decoration: none;
		color: var(--color-text-muted);
		padding: var(--space-1) 0;
		border-bottom: 2px solid transparent;
	}
	nav a[aria-current='page'] {
		color: var(--color-text);
		border-bottom-color: var(--color-accent);
	}
	main {
		max-width: var(--layout-max);
		margin: 0 auto;
		padding: var(--space-5) var(--gutter);
	}
</style>
