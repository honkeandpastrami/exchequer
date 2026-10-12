<script lang="ts">
	// A line or column chart drawn in SVG. Colors and sizes come from the
	// theme tokens (`--chart-*`); values stay strings until placed in pixels.
	import Amount from '../components/Amount.svelte';
	import { formatAmount } from '../money';
	import { axis, labelIndexes, position, type Series } from './scale';

	let {
		labels,
		series,
		kind = 'line',
		code
	}: {
		/** One per point along the x axis. */
		labels: string[];
		/** Each has one value per label. */
		series: Series[];
		kind?: 'line' | 'columns';
		/** Commodity code shown in the hover box. */
		code?: string;
	} = $props();

	const M = { top: 12, right: 12, bottom: 28, left: 72 };
	let width = $state(0);
	let height = $state(0);
	let hover: number | null = $state(null);

	let a = $derived(axis(series.flatMap((s) => s.values)));
	let plotW = $derived(Math.max(0, width - M.left - M.right));
	let plotH = $derived(Math.max(0, height - M.top - M.bottom));
	let n = $derived(labels.length);
	let band = $derived(n ? plotW / n : 0);

	const color = (i: number) => `var(--chart-${(i % 8) + 1})`;
	const y = (v: string) => M.top + plotH - position(v, a, plotH);
	// Lines put points on band centers, so lines and columns share an x scale.
	const x = (i: number) => M.left + band * (i + 0.5);

	function path(values: string[]): string {
		return values.map((v, i) => `${i ? 'L' : 'M'}${x(i)},${y(v)}`).join('');
	}

	function column(si: number, i: number) {
		const gap = Math.min(4, band * 0.15);
		const w = (band - 2 * gap) / series.length;
		return M.left + band * i + gap + w * si;
	}

	function moved(e: PointerEvent) {
		const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
		const i = Math.floor((e.clientX - rect.left - M.left) / band);
		hover = band && i >= 0 && i < n ? i : null;
	}
</script>

<figure class="chart">
	<div class="plot" bind:clientWidth={width} bind:clientHeight={height}>
		{#if width && n}
			<svg
				{width}
				{height}
				role="img"
				aria-label={series.map((s) => s.name).join(', ')}
				onpointermove={moved}
				onpointerleave={() => (hover = null)}
			>
				{#each a.ticks as t (t)}
					<line
						class="grid"
						class:zero={t.match(/^-?0(\.0+)?$/)}
						x1={M.left}
						x2={width - M.right}
						y1={y(t)}
						y2={y(t)}
					/>
					<text class="tick" x={M.left - 6} y={y(t)} text-anchor="end" dominant-baseline="middle"
						>{formatAmount(t)}</text
					>
				{/each}
				{#each labelIndexes(n, Math.max(2, Math.floor(plotW / 80))) as i (i)}
					<text class="tick" x={x(i)} y={height - 8} text-anchor="middle">{labels[i]}</text>
				{/each}
				{#if hover !== null}
					<rect class="hover" x={M.left + band * hover} y={M.top} width={band} height={plotH} />
				{/if}
				{#each series as s, si (s.name)}
					{#if kind === 'line'}
						<path d={path(s.values)} fill="none" stroke={color(si)} />
						{#if n === 1 || hover !== null}
							{@const i = hover ?? 0}
							<circle cx={x(i)} cy={y(s.values[i])} r="3.5" fill={color(si)} />
						{/if}
					{:else}
						{#each s.values as v, i (i)}
							{@const top = Math.min(y(v), y('0'))}
							<rect
								x={column(si, i)}
								y={top}
								width={Math.max(1, (band - 2 * Math.min(4, band * 0.15)) / series.length)}
								height={Math.abs(y(v) - y('0'))}
								fill={color(si)}
							/>
						{/each}
					{/if}
				{/each}
			</svg>
			{#if hover !== null}
				<div
					class="tip"
					style:left="{x(hover)}px"
					style:transform={x(hover) > width / 2
						? 'translateX(calc(-100% - 12px))'
						: 'translateX(12px)'}
				>
					<strong>{labels[hover]}</strong>
					{#each series as s, si (s.name)}
						<div class="tip-row">
							<span class="swatch" style:background={color(si)}></span>
							<span class="name">{s.name}</span>
							<Amount value={s.values[hover]} {code} />
						</div>
					{/each}
				</div>
			{/if}
		{/if}
	</div>
	{#if series.length > 1}
		<figcaption class="legend">
			{#each series as s, si (s.name)}
				<span><span class="swatch" style:background={color(si)}></span>{s.name}</span>
			{/each}
		</figcaption>
	{/if}
</figure>

<style>
	.chart {
		margin: 0;
	}
	.plot {
		position: relative;
		height: var(--chart-height);
	}
	svg {
		display: block;
		overflow: visible;
	}
	path {
		stroke-width: 2;
		stroke-linejoin: round;
	}
	.grid {
		stroke: var(--chart-grid);
	}
	.grid.zero {
		stroke: var(--color-border);
		stroke-width: 1.5;
	}
	.tick {
		font-size: var(--text-xs);
		fill: var(--color-text-muted);
		font-variant-numeric: var(--numeric);
	}
	.hover {
		fill: var(--color-surface-alt);
		opacity: 0.6;
	}
	.tip {
		position: absolute;
		top: var(--space-2);
		z-index: 1;
		padding: var(--space-2) var(--space-3);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-1);
		font-size: var(--text-sm);
		pointer-events: none;
		white-space: nowrap;
	}
	.tip-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.tip-row .name {
		flex: 1;
		color: var(--color-text-muted);
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-4);
		margin-top: var(--space-2);
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.swatch {
		display: inline-block;
		width: 0.75em;
		height: 0.75em;
		margin-right: var(--space-1);
		border-radius: 2px;
	}
</style>
