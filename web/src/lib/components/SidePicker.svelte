<script lang="ts">
	// One side of a split, typed: an account or someone outside the
	// household, chosen alike from what matches as you type (ux.md §10).
	// Text that's no account is a counterparty's name, a new one if no one
	// has it; "Unknown" is someone outside, unknown who.
	import type { Reference } from '../reference';
	import { legOf, legText, sideChoices, UNKNOWN, type LegDraft } from '../views/editor';

	let {
		leg,
		set,
		ref,
		id,
		label,
		skip,
		offer
	}: {
		leg: LegDraft;
		set: (l: LegDraft) => void;
		ref: Reference;
		id: string;
		label: string;
		/** An account not offered: the bank's own. */
		skip?: string;
		/** A suggestion, offered first: picking it applies it. */
		offer?: { label: string; apply: () => void };
	} = $props();

	/** Text being typed; `null` while not editing, showing the side. */
	let text: string | null = $state(null);
	/** Whether the text was typed since focusing: else every choice shows. */
	let typed = $state(false);
	let open = $state(false);
	let active = $state(-1);

	const shown = $derived(text ?? legText(leg, ref));

	type Choice = { text: string; note: string; pick: () => void };
	const choices: Choice[] = $derived.by(() => {
		const query = typed ? shown : '';
		const to = (t: string) => () => set(legOf(t, ref));
		const out: Choice[] = [];
		const hit = (t: string) =>
			!query.trim() || t.toLowerCase().includes(query.trim().toLowerCase());
		if (offer && hit(offer.label))
			out.push({ text: offer.label, note: 'suggested', pick: offer.apply });
		for (const c of sideChoices(query, ref, skip))
			if (c.text !== offer?.label)
				out.push({ text: c.text, note: c.kind === 'account' ? 'account' : '', pick: to(c.text) });
		const exact = out.some((c) => c.text.toLowerCase() === query.trim().toLowerCase());
		if (query.trim() && !exact)
			out.push({ text: query.trim(), note: 'new counterparty', pick: to(query) });
		if (offer?.label !== UNKNOWN && hit(UNKNOWN))
			out.push({ text: UNKNOWN, note: 'outside the household', pick: to(UNKNOWN) });
		return out;
	});

	function choose(c: Choice) {
		c.pick();
		text = null;
		typed = false;
		open = false;
		active = -1;
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			open = true;
			const n = choices.length;
			if (n) active = e.key === 'ArrowDown' ? (active + 1) % n : active <= 0 ? n - 1 : active - 1;
		} else if (e.key === 'Enter' && open && active >= 0 && choices[active]) {
			e.preventDefault();
			choose(choices[active]);
		} else if (e.key === 'Escape' && open) {
			e.stopPropagation();
			open = false;
			active = -1;
		}
	}
</script>

<div class="picker">
	<input
		{id}
		role="combobox"
		autocomplete="off"
		aria-label={label}
		aria-expanded={open && choices.length > 0}
		aria-controls="{id}-list"
		aria-autocomplete="list"
		aria-activedescendant={open && active >= 0 ? `${id}-${active}` : undefined}
		placeholder="Account or counterparty"
		value={shown}
		onfocus={(e) => {
			text = legText(leg, ref);
			typed = false;
			open = true;
			e.currentTarget.select();
		}}
		oninput={(e) => {
			text = e.currentTarget.value;
			typed = true;
			open = true;
			active = -1;
			set(legOf(text, ref));
		}}
		onblur={() => {
			text = null;
			typed = false;
			open = false;
			active = -1;
		}}
		onkeydown={keydown}
	/>
	{#if open && choices.length}
		<ul class="list" id="{id}-list" role="listbox" aria-label={label}>
			{#each choices as c, k (k)}
				<!-- Mousedown, not click: picking mustn't blur the box first. -->
				<li
					id="{id}-{k}"
					role="option"
					aria-selected={k === active}
					class:active={k === active}
					onmousedown={(e) => {
						e.preventDefault();
						choose(c);
					}}
				>
					<span class="text">{c.text}</span>
					{#if c.note}<span class="note">{c.note}</span>{/if}
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.picker {
		position: relative;
	}
	input {
		width: 100%;
	}
	.list {
		position: absolute;
		z-index: 10;
		top: calc(100% + 2px);
		left: 0;
		min-width: 100%;
		width: max-content;
		max-width: min(24rem, 90vw);
		max-height: 16rem;
		margin: 0;
		padding: var(--space-1) 0;
		overflow-y: auto;
		list-style: none;
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-1);
	}
	li {
		display: flex;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-1) var(--space-2);
		cursor: pointer;
	}
	li:hover,
	li.active {
		background: var(--color-surface-alt);
	}
	.text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.note {
		flex: none;
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
</style>
