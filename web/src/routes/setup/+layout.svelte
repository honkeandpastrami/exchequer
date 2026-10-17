<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const tabs = [
		{ href: resolve('/setup/accounts'), label: 'Accounts', route: '/setup/accounts' },
		{ href: resolve('/setup/parties'), label: 'Parties', route: '/setup/parties' },
		{ href: resolve('/setup/categories'), label: 'Categories', route: '/setup/categories' },
		{
			href: resolve('/setup/counterparties'),
			label: 'Counterparties',
			route: '/setup/counterparties'
		},
		{ href: resolve('/setup/tags'), label: 'Tags', route: '/setup/tags' },
		{ href: resolve('/setup/commodities'), label: 'Commodities', route: '/setup/commodities' },
		{ href: resolve('/setup/prices'), label: 'Prices', route: '/setup/prices' }
	];
</script>

<nav class="tabs">
	{#each tabs as t (t.route)}
		<a href={t.href} aria-current={page.route.id === t.route ? 'page' : undefined}>{t.label}</a>
	{/each}
</nav>
{@render children()}

<style>
	.tabs {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-bottom: var(--space-4);
		font-size: var(--text-sm);
	}
	.tabs a {
		padding: var(--space-1) var(--space-3);
		border-radius: var(--radius-sm);
		text-decoration: none;
		color: var(--color-text-muted);
	}
	.tabs a[aria-current='page'] {
		background: var(--color-surface-alt);
		color: var(--color-text);
	}
</style>
