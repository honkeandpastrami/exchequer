<script lang="ts">
	let { error }: { error: unknown } = $props();
</script>

<p class="failure" role="alert">{error instanceof Error ? error.message : String(error)}</p>

<style>
	.failure {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-sm);
		background: var(--color-error-bg);
		color: var(--color-negative);
	}
</style>
