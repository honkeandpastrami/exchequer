<script lang="ts">
	import type { Status } from '../views/settlement';

	let { status }: { status: Status } = $props();

	const marks: Record<Status, { mark: string; label: string }> = {
		open: { mark: '○', label: 'Open' },
		settled: { mark: '✓', label: 'Settled' },
		'paying-down': { mark: '↺', label: 'Paying down a past difference' },
		difference: { mark: '⚠', label: 'Closed with a difference' }
	};
</script>

<span class="status {status}" title={marks[status].label}
	>{marks[status].mark}<span class="label">{marks[status].label}</span></span
>

<style>
	.status {
		white-space: nowrap;
	}
	.label {
		margin-left: var(--space-1);
		font-size: var(--text-xs);
	}
	.open {
		color: var(--color-text-muted);
	}
	.settled {
		color: var(--color-positive);
	}
	.paying-down {
		color: var(--color-accent);
	}
	.difference {
		color: var(--color-warning);
	}
</style>
