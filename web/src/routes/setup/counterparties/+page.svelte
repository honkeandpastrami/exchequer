<script lang="ts">
	import { counterparties } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import NamedList from '#lib/components/NamedList.svelte';
	import { referenceState } from '#lib/setup.svelte.ts';

	const { state: s, reload } = referenceState();
</script>

<h1>Counterparties</h1>
{#if s.ref}
	<NamedList
		items={[...s.ref.counterparties.values()]}
		resource={counterparties}
		noun="counterparty"
		filter="counterparty_id"
		onchange={reload}
	/>
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}
