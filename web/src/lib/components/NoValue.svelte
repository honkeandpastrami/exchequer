<script lang="ts">
	// Where a value would be, something that isn't one (docs/ux.md §4): a
	// deliberate `none`, or an `unknown`, not set. A tag, so it can't be
	// mistaken for data, a name like "Unknown" included.
	let { kind }: { kind: 'none' | 'unknown' } = $props();
</script>

<span class="tag {kind}">{kind}</span>

<style>
	.tag {
		display: inline-block;
		padding: 0 var(--space-1);
		border: var(--border-width) solid var(--color-border);
		border-radius: 999px;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		line-height: 1.4;
		text-transform: lowercase;
		white-space: nowrap;
		vertical-align: baseline;
	}
	/* Dark grey: the row's colour says whether it needs you. */
	.unknown {
		border-color: var(--color-text-muted);
		background: var(--color-surface-alt);
		color: var(--color-text);
	}
</style>
