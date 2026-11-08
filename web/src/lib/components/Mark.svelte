<script lang="ts" module>
	/** What a status says, by its symbol and colour (docs/ux.md §6). */
	export type Tone = 'ok' | 'warning' | 'error' | 'open' | 'progress' | 'off';
</script>

<script lang="ts">
	// `quiet`: the word is for screen readers only, where the words beside the
	// symbol already say what it is.
	let { tone, label, quiet = false }: { tone: Tone; label: string; quiet?: boolean } = $props();

	const symbols: Record<Tone, string> = {
		ok: '✓',
		warning: '⚠',
		error: '✗',
		open: '○',
		progress: '↺',
		off: '⊘'
	};
</script>

<span class="mark {tone}" title={label}
	><span aria-hidden="true">{symbols[tone]}</span><span class="label" class:quiet>{label}</span
	></span
>

<style>
	.mark {
		white-space: nowrap;
	}
	.label {
		margin-left: var(--space-1);
		font-size: var(--text-xs);
	}
	.quiet {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
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
	.off {
		color: var(--color-text-muted);
	}
</style>
