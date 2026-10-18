<script lang="ts">
	import { categories } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import NamedList from '#lib/components/NamedList.svelte';
	import { referenceState } from '#lib/setup.svelte.ts';

	const { state: s, reload } = referenceState();
</script>

{#if s.ref}
	<NamedList
		title="Categories"
		items={[...s.ref.categories.values()]}
		resource={categories}
		noun="category"
		filter="category_id"
		onchange={reload}
	/>
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}
