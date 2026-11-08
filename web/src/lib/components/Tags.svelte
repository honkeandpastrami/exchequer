<script lang="ts">
	// A transaction's tags as chips (ux.md §10): typing offers the existing
	// tags, Enter adds one, × removes it. A name no tag has is a new tag,
	// made when the transaction is saved.
	let {
		tags = $bindable(),
		known,
		id,
		label
	}: {
		tags: string[];
		/** The tags there are, by name, offered as you type. */
		known: string[];
		id: string;
		label: string;
	} = $props();

	let text = $state('');

	function add() {
		const t = text.trim();
		if (t && !tags.some((x) => x.toLowerCase() === t.toLowerCase())) tags = [...tags, t];
		text = '';
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ',') {
			e.preventDefault();
			add();
		} else if (e.key === 'Backspace' && !text && tags.length) {
			tags = tags.slice(0, -1);
		}
	}
</script>

<div class="tags">
	{#each tags as t (t)}
		<span class="chip">
			<span class="name">{t}</span>
			<button
				type="button"
				class="remove"
				aria-label="Remove tag {t}"
				onclick={() => (tags = tags.filter((x) => x !== t))}>×</button
			>
		</span>
	{/each}
	<input
		{id}
		aria-label={label}
		list="{id}-known"
		placeholder="Add a tag…"
		maxlength="100"
		bind:value={text}
		onkeydown={keydown}
		onchange={add}
	/>
	<datalist id="{id}-known">
		{#each known.filter((k) => !tags.includes(k)) as k (k)}<option value={k}></option>{/each}
	</datalist>
</div>

<style>
	.tags {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
	.chip {
		display: inline-flex;
		align-items: center;
		min-height: var(--target);
		padding-left: var(--space-3);
		border: var(--border-width) solid var(--color-border);
		border-radius: 999px;
		background: var(--color-surface-alt);
		overflow-wrap: anywhere;
	}
	.remove {
		min-width: var(--target);
		padding: 0;
		border: none;
		border-radius: 999px;
		background: transparent;
		color: var(--color-text-muted);
	}
	.remove:hover {
		background: var(--color-border);
	}
	input {
		flex: 1;
		min-width: 10rem;
	}
</style>
