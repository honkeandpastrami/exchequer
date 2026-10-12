<script lang="ts">
	import { page } from '$app/state';
	import TransactionEditor from '#lib/components/TransactionEditor.svelte';
</script>

<h1>New transaction</h1>
{#key page.url.hash}
	<TransactionEditor id={null} />
{/key}
