<script lang="ts">
	import { tags } from '#lib/api/resources.ts';
	import Failure from '#lib/components/Failure.svelte';
	import NamedList from '#lib/components/NamedList.svelte';
	import { referenceState } from '#lib/setup.svelte.ts';

	const { state: s, reload } = referenceState();
</script>

<h1>Tags</h1>
{#if s.ref}
	<NamedList
		items={[...s.ref.tags.values()]}
		resource={tags}
		noun="tag"
		filter="tag_id"
		onchange={reload}
	/>
{:else if s.failure}
	<Failure error={s.failure} />
{:else}
	<p class="muted">Loading…</p>
{/if}
