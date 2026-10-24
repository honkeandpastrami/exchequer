<script lang="ts">
	import type { Snippet } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '../api/client';
	import { signOut } from '../session.svelte';
	import { version } from '../version';

	let { children }: { children: Snippet } = $props();

	const current = (route: string) =>
		route === '/' ? page.route.id === '/' : !!page.route.id?.startsWith(route);

	/** Ends a passkey session on the server too; an API token stays valid. */
	async function leave() {
		await api.DELETE('/api/v1/me/session').catch(() => {});
		signOut();
	}

	const sections = [
		{ href: resolve('/'), label: 'Accounts', route: '/' },
		{ href: resolve('/transactions'), label: 'Transactions', route: '/transactions' },
		{ href: resolve('/imports'), label: 'Imports', route: '/imports' },
		{ href: resolve('/settlement'), label: 'Settlement', route: '/settlement' },
		{ href: resolve('/reports'), label: 'Reports', route: '/reports' },
		{ href: resolve('/setup'), label: 'Setup', route: '/setup' }
	];
</script>

<header class="bar">
	<span class="brand">Exchequer <span class="version">{version}</span></span>
	<nav>
		{#each sections as s (s.route)}
			<a href={s.href} aria-current={current(s.route) ? 'page' : undefined}>{s.label}</a>
		{/each}
	</nav>
	<button class="quiet" onclick={leave}>Sign out</button>
</header>
<main>
	{@render children()}
</main>

<style>
	.bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-5);
		padding: var(--space-2) var(--gutter);
		background: var(--color-surface);
		border-bottom: var(--border-width) solid var(--color-border);
	}
	.brand {
		font-family: var(--font-display);
		font-size: var(--text-lg);
		font-weight: var(--weight-strong);
	}
	.version {
		font-family: var(--font-ui);
		font-size: var(--text-xs);
		font-weight: var(--weight-normal);
		color: var(--color-text-muted);
	}
	nav {
		display: flex;
		gap: var(--space-4);
		flex: 1;
		overflow-x: auto;
	}
	/* Narrow screens: brand and sign-out on top, the sections below. */
	@media (max-width: 40rem) {
		nav {
			order: 1;
			flex-basis: 100%;
		}
		.brand {
			flex: 1;
		}
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
