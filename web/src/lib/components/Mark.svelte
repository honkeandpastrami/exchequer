<script lang="ts" module>
	/** What a status says, by its symbol and colour (docs/ux.md §6). */
	export type Tone = 'ok' | 'warning' | 'error' | 'open' | 'progress';
</script>

<script lang="ts">
	let { tone, label }: { tone: Tone; label: string } = $props();

	const symbols: Record<Tone, string> = {
		ok: '✓',
		warning: '⚠',
		error: '✗',
		open: '○',
		progress: '↺'
	};
</script>

<span class="mark {tone}" title={label}
	><span aria-hidden="true">{symbols[tone]}</span><span class="label">{label}</span></span
>

<style>
	.mark {
		white-space: nowrap;
	}
	.label {
		margin-left: var(--space-1);
		font-size: var(--text-xs);
	}
	.ok {
		color: var(--color-positive);
	}
	.warning {
		color: var(--color-warning);
	}
	.error {
		color: var(--color-negative);
	}
	.open {
		color: var(--color-text-muted);
	}
	.progress {
		color: var(--color-accent);
	}
</style>
