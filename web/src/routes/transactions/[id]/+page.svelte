<script lang="ts">
	import { page } from '$app/state';
	import TransactionList from '#lib/components/TransactionList.svelte';
</script>

<!-- The list, showing only this transaction, its form open. -->
{#key page.params.id}
	<TransactionList only={page.params.id ?? null} />
{/key}
