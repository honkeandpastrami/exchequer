<script lang="ts">
	// Where a value would be, something that isn't one (docs/ux.md §4): a
	// deliberate `none`, or an `unknown` that still needs an answer. A tag,
	// so it can't be mistaken for data.
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
	.unknown {
		border-color: var(--color-warning);
		background: var(--color-warning-bg);
		color: var(--color-warning);
	}
</style>
