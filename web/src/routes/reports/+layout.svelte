<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const tabs = [
		{ href: resolve('/reports/balances'), label: 'Balance history', route: '/reports/balances' },
		{ href: resolve('/reports/spending'), label: 'By category', route: '/reports/spending' },
		{ href: resolve('/reports/pool'), label: 'Household pool', route: '/reports/pool' }
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
