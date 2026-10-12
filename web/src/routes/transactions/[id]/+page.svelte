<script lang="ts">
	import { page } from '$app/state';
	import TransactionEditor from '#lib/components/TransactionEditor.svelte';
</script>

<h1>Edit transaction</h1>
{#key page.params.id}
	<TransactionEditor id={page.params.id ?? null} />
{/key}
