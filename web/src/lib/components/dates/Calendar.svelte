<script lang="ts">
	// The calendar every date control opens (docs/ux.md §11): days, months or
	// years in a grid, zooming out from its header and back in down to the
	// granularity asked for. It floats under its anchor like the reason
	// bubble (SuggestionBadge), so nothing under it moves.
	import { onMount, tick, untrack } from 'svelte';
	import { current, type Granularity } from '../../dates';
	import { pixels, rootFont } from '../../layout';
	import {
		WEEKDAYS,
		beyond,
		cells,
		page,
		pick,
		title,
		viewOf,
		zoomOut,
		type View
	} from '../../views/calendar';

	let {
		value,
		granularity,
		max,
		anchor,
		onpick,
		onclose
	}: {
		/** The value shown as picked; empty opens at today. */
		value: string;
		granularity: Granularity;
		/** The latest value allowed, at `granularity`. */
		max?: string;
		/** What it opens under. */
		anchor: HTMLElement;
		onpick: (value: string) => void;
		onclose: () => void;
	} = $props();

	// Where it opens; it's opened afresh each time, so later changes don't move it.
	const start = untrack(() => value || current(granularity));
	let view: View = $state(untrack(() => viewOf(start, granularity)));
	/** The cell keyboard focus is on. */
	let focused = $state(start);
	let box: HTMLElement | undefined = $state();

	let grid = $derived(cells(view));
	let columns = $derived(view.level === 'day' ? 7 : 3);

	/** Where `focused` sits in the current view. */
	const inView = (v: string) => v.slice(0, view.at.length) === view.at || view.level === 'year';

	function place() {
		if (!box) return;
		const at = anchor.getBoundingClientRect();
		const gutter = pixels(getComputedStyle(box).getPropertyValue('--gutter'), rootFont());
		const room = document.documentElement.clientWidth - gutter;
		box.style.top = `${at.bottom + 4}px`;
		box.style.left = `${Math.max(gutter, Math.min(at.left, room - box.offsetWidth))}px`;
	}

	async function focusCell() {
		await tick();
		box?.querySelector<HTMLButtonElement>(`[data-value="${focused}"]`)?.focus();
	}

	onMount(() => {
		box?.showPopover();
		place();
		focusCell();
		const away = (e: PointerEvent) => {
			const t = e.target as Node;
			if (!box?.contains(t) && !anchor.contains(t)) onclose();
		};
		// It sits where its anchor was: scrolling would leave it behind.
		const gone = () => onclose();
		window.addEventListener('pointerdown', away);
		window.addEventListener('scroll', gone, { capture: true, passive: true });
		window.addEventListener('resize', gone);
		return () => {
			window.removeEventListener('pointerdown', away);
			window.removeEventListener('scroll', gone, { capture: true });
			window.removeEventListener('resize', gone);
		};
	});

	function choose(v: string) {
		const next = pick(view, v, granularity);
		if (typeof next === 'string') {
			onpick(next);
			onclose();
			return;
		}
		view = next;
		focused = cells(next).find((c) => c)!.value;
		focusCell();
	}

	function turn(n: number) {
		view = page(view, n);
		const first = grid.find((c) => c);
		if (first && !inView(focused)) focused = first.value;
	}

	function out() {
		view = zoomOut(view);
		focused = cells(view).find((c) => c && focused.startsWith(c.value))?.value ?? focused;
		focusCell();
	}

	function key(e: KeyboardEvent) {
		const moves: Record<string, number> = {
			ArrowLeft: -1,
			ArrowRight: 1,
			ArrowUp: -columns,
			ArrowDown: columns
		};
		if (e.key === 'Escape') {
			e.preventDefault();
			onclose();
			anchor.focus();
		} else if (e.key === 'PageUp' || e.key === 'PageDown') {
			e.preventDefault();
			turn(e.key === 'PageUp' ? -1 : 1);
			focusCell();
		} else if (e.key in moves) {
			e.preventDefault();
			const values = grid.map((c) => c?.value);
			const i = values.indexOf(focused) + moves[e.key];
			if (i >= 0 && i < values.length && values[i]) focused = values[i]!;
			else {
				view = page(view, moves[e.key] < 0 ? -1 : 1);
				const real = cells(view).filter((c) => c);
				focused = (moves[e.key] < 0 ? real[real.length - 1] : real[0])!.value;
			}
			focusCell();
		}
	}
</script>

<div
	bind:this={box}
	class="calendar"
	popover="manual"
	role="dialog"
	aria-label="Pick a date"
	tabindex="-1"
	onkeydown={key}
>
	<div class="header">
		<button type="button" class="quiet" aria-label="Earlier" onclick={() => turn(-1)}>‹</button>
		<button
			type="button"
			class="quiet title"
			disabled={view.level === 'year'}
			onclick={out}
			aria-label="{title(view)}: zoom out">{title(view)}</button
		>
		<button
			type="button"
			class="quiet"
			aria-label="Later"
			disabled={max !== undefined && beyond(page(view, 1).at, max)}
			onclick={() => turn(1)}>›</button
		>
	</div>
	<div class="grid" style:--columns={columns}>
		{#if view.level === 'day'}
			{#each WEEKDAYS as d (d)}<span class="weekday">{d}</span>{/each}
		{/if}
		{#each grid as c, i (c?.value ?? `blank-${i}`)}
			{#if c}
				<button
					type="button"
					class="cell"
					class:picked={value !== '' && value.startsWith(c.value) && view.level === granularity}
					data-value={c.value}
					tabindex={c.value === focused ? 0 : -1}
					disabled={beyond(c.value, max)}
					onclick={() => choose(c.value)}>{c.label}</button
				>
			{:else}
				<span></span>
			{/if}
		{/each}
	</div>
</div>

<style>
	/* In the top layer, placed by `place`: nothing it covers moves. One
	   size throughout, the size of the stepper that opens it. */
	.calendar {
		inset: auto;
		margin: 0;
		padding: var(--space-2);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		background: var(--color-surface);
		color: var(--color-text);
		box-shadow: var(--shadow-1);
		font: var(--text-sm) / var(--leading) var(--font-ui);
		font-variant-numeric: var(--numeric);
	}
	.header {
		display: grid;
		grid-template-columns: auto 1fr auto;
		gap: var(--space-1);
		margin-bottom: var(--space-1);
	}
	.title:disabled {
		opacity: 1;
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(var(--columns), 1fr);
	}
	.weekday {
		color: var(--color-text-muted);
		text-align: center;
	}
	/* A cell is its grid square, edge to edge: the click area is the room
	   that's there, with no padding of its own. */
	.cell {
		min-width: 2.25em;
		padding: var(--space-1) 0;
		border: 0;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-text);
		text-align: center;
	}
	.cell:hover:not(:disabled) {
		background: var(--color-surface-alt);
	}
	.cell.picked {
		background: var(--color-accent);
		color: var(--color-on-accent);
	}
	.grid:not(:has(.weekday)) .cell {
		min-width: 4em;
	}
</style>
