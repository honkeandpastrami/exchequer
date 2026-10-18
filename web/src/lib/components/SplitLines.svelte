<script lang="ts" module>
	/** A field a suggestion can be for. */
	export type Field = 'other' | 'category' | 'purpose';
	/** A suggestion a field's dropdown offers: picking it applies it. */
	export type Offer = { label: string; apply: () => void };
</script>

<script lang="ts">
	// A transaction's splits, one line each (ux.md §10). Fields a split can't
	// have stay in their place, disabled (see `preview`); a conversion's
	// received amount and a settled month go on a second line, under ⋯.
	// Staged bank rows use it too: their account's side is fixed, the amount
	// is the bank's until split, and each field may offer a suggestion and
	// say where its value came from.
	import type { Snippet } from 'svelte';
	import { kinds, partyKinds, rank } from '../labels';
	import { minus, sign } from '../money';
	import { accountLabel } from '../names';
	import { sorted, type Reference } from '../reference';
	import {
		blankSplit,
		OUTSIDE,
		preview,
		splitTotals,
		UNSET,
		type LegDraft,
		type SplitDraft
	} from '../views/editor';
	import Amount from './Amount.svelte';
	import Mark from './Mark.svelte';

	let {
		splits = $bindable(),
		ref,
		commodity,
		fixed,
		expected,
		canAdd = true,
		more: withMore = true,
		offers,
		status
	}: {
		splits: SplitDraft[];
		ref: Reference;
		/** A new split's commodity when there's none to copy. */
		commodity: string;
		/** A bank row's account: its side is shown, not chosen, as is the commodity. */
		fixed?: { account: string; side: 'from' | 'to' };
		/** What the splits must add up to: a bank row's amount, unsigned. One
		 * split is the whole of it, and its amount can't change. */
		expected?: string;
		/** Whether Add split is offered. */
		canAdd?: boolean;
		/** Whether ⋯ (another commodity, a settled month) is offered. */
		more?: boolean;
		/** The suggestions split `i`'s fields offer. */
		offers?: (i: number) => Partial<Record<Field, Offer>>;
		/** A line under split `i`'s field: where its value came from. */
		status?: Snippet<[number, Field]>;
	} = $props();

	/** The select value of an offered suggestion. */
	const SUGGESTED = 'suggested';

	let more: boolean[] = $state([]);
	// Several forms may be open at once.
	const uid = $props.id();

	const owners = $derived(
		[...ref.parties.values()].sort(
			(a, b) => rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)
		)
	);
	const accountsOf = (owner: string, keep: string) =>
		sorted(ref.accounts).filter(
			(a) => a.owner_id === owner && (!a.archived || a.id === keep) && a.id !== fixed?.account
		);
	const commodities = $derived(
		[...ref.commodities.values()].sort((a, b) => a.code.localeCompare(b.code))
	);
	const code = (id: string) => ref.commodities.get(id)?.code;

	const totals = $derived(
		splitTotals(splits).map((t) => ({ code: code(t.commodity), value: t.value }))
	);
	/** What's left to split of `expected`: positive, short; negative, over. */
	const left = $derived(
		expected === undefined ? undefined : minus(expected, splitTotals(splits)[0]?.value ?? '0')
	);

	/** The other side: the one that isn't the bank account's. */
	const otherOf = (s: SplitDraft): LegDraft => (fixed?.side === 'from' ? s.to : s.from);
	/** What decides which fields a split has: an unchosen other side of a
	 * bank row is someone outside, until chosen. */
	function previewOf(s: SplitDraft) {
		if (!fixed || otherOf(s).account) return preview(s, ref);
		const outside = { account: OUTSIDE, outside: '' };
		return preview(fixed.side === 'from' ? { ...s, to: outside } : { ...s, from: outside }, ref);
	}

	function pick(i: number, field: Field, value: string, set: (v: string) => void) {
		const offer = offers?.(i)[field];
		if (value === SUGGESTED && offer) offer.apply();
		else set(value);
	}

	function add() {
		splits.push(blankSplit(commodity, splits.at(-1)));
	}
	function remove(i: number) {
		splits.splice(i, 1);
		more.splice(i, 1);
		// Unsplit, a bank row's one split is the whole of it.
		if (expected !== undefined && splits.length === 1) splits[0].quantity = expected;
	}
</script>

{#snippet side(label: string, s: SplitDraft, l: LegDraft, i: number)}
	{@const n = i + 1}
	{@const offer = offers?.(i).other}
	<div class="field side">
		<label for="{uid}-{i}-{label}">{label}</label>
		{#if fixed && l !== otherOf(s)}
			{@const a = ref.accounts.get(fixed.account)}
			<span class="fixed" id="{uid}-{i}-{label}">{a ? accountLabel(a, ref) : '?'}</span>
		{:else}
			<select
				id="{uid}-{i}-{label}"
				value={l.account}
				onchange={(e) => pick(i, 'other', e.currentTarget.value, (v) => (l.account = v))}
				aria-label="Split {n}: {label}"
			>
				<option value="" disabled>Choose…</option>
				{#if offer}<option value={SUGGESTED}>{offer.label} (suggested)</option>{/if}
				{#each owners as o (o.id)}
					{@const list = accountsOf(o.id, l.account)}
					{#if list.length}
						<optgroup label={o.name}>
							<!-- The full label: a closed picker shows only the option. -->
							{#each list as a (a.id)}<option value={a.id}>{accountLabel(a, ref)}</option>{/each}
						</optgroup>
					{/if}
				{/each}
				<option value={OUTSIDE}>Outside the household…</option>
			</select>
			{#if l.account === OUTSIDE}
				<input
					bind:value={l.outside}
					list="{uid}-counterparties"
					placeholder="Counterparty (blank if unknown)"
					aria-label="Split {n}: {label}, counterparty"
				/>
			{/if}
			{#if status}<div class="status">{@render status(i, 'other')}</div>{/if}
		{/if}
	</div>
{/snippet}

<datalist id="{uid}-counterparties">
	{#each sorted(ref.counterparties) as c (c.id)}<option value={c.name}></option>{/each}
</datalist>

<div class="lines">
	<!-- Wide enough: one line per split under one set of headings. -->
	<div class="heads" aria-hidden="true">
		<span></span><span>From</span><span></span><span>To</span><span>Amount</span><span
			>Category</span
		><span>For</span><span>Note</span><span class="tools-space"></span>
	</div>
	<ol class="splits">
		{#each splits as split, i (i)}
			{@const p = previewOf(split)}
			{@const n = i + 1}
			{@const k = p.kind ? kinds[p.kind].toLowerCase() : ''}
			{@const notFor = k
				? `Not for ${/^[aeiou]/.test(k) ? 'an' : 'a'} ${k}`
				: 'Choose both sides first'}
			{@const offer = offers?.(i) ?? {}}
			{@const whole = expected !== undefined && splits.length === 1}
			<li class="split">
				<div class="line">
					<span class="n" aria-hidden="true">{n}</span>
					{@render side('From', split, split.from, i)}
					<span class="arrow" aria-hidden="true">→</span>
					{@render side('To', split, split.to, i)}
					<label class="field">
						<span>Amount</span>
						<span class="money">
							<input
								class="qty"
								inputmode="decimal"
								bind:value={split.quantity}
								placeholder="0.00"
								disabled={whole}
								aria-label="Split {n}: amount"
							/>
							<select
								bind:value={split.commodity}
								disabled={!!fixed}
								aria-label="Split {n}: commodity"
							>
								{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
							</select>
						</span>
					</label>
					<div class="field">
						<label for="{uid}-{i}-category">Category</label>
						{#if p.category}
							<select
								id="{uid}-{i}-category"
								class="pick"
								value={split.category}
								onchange={(e) =>
									pick(i, 'category', e.currentTarget.value, (v) => (split.category = v))}
								aria-label="Split {n}: category"
							>
								{#if split.category === UNSET}<option value={UNSET} disabled>Choose…</option>{/if}
								{#if offer.category}<option value={SUGGESTED}
										>{offer.category.label} (suggested)</option
									>{/if}
								<option value="">None</option>
								{#each sorted(ref.categories) as c (c.id)}<option value={c.id}>{c.name}</option
									>{/each}
							</select>
							{#if status}<div class="status">{@render status(i, 'category')}</div>{/if}
						{:else}
							<select
								id="{uid}-{i}-category"
								class="pick"
								disabled
								title={notFor}
								aria-label="Split {n}: category"><option>Doesn't apply</option></select
							>
						{/if}
					</div>
					<div class="field">
						<label for="{uid}-{i}-purpose">For</label>
						{#if p.purpose}
							{@const unset = split.purpose === UNSET || (!split.purpose && !p.defaultPurpose)}
							<!-- No purpose picked: the default party, shown by name like any other. -->
							<select
								id="{uid}-{i}-purpose"
								class="pick"
								value={split.purpose === UNSET ? UNSET : split.purpose || p.defaultPurpose || UNSET}
								onchange={(e) =>
									pick(i, 'purpose', e.currentTarget.value, (v) => (split.purpose = v))}
								aria-label="Split {n}: for"
							>
								{#if unset}<option value={UNSET} disabled>Choose…</option>{/if}
								{#if offer.purpose}<option value={SUGGESTED}
										>{offer.purpose.label} (suggested)</option
									>{/if}
								{#each owners as o (o.id)}<option value={o.id}>{o.name}</option>{/each}
							</select>
							{#if status}<div class="status">{@render status(i, 'purpose')}</div>{/if}
						{:else}
							<select
								id="{uid}-{i}-purpose"
								class="pick"
								disabled
								title={notFor}
								aria-label="Split {n}: for"><option>Doesn't apply</option></select
							>
						{/if}
					</div>
					<label class="field grow">
						<span>Note</span>
						<input bind:value={split.description} aria-label="Split {n}: note" />
					</label>
					<span class="tools">
						{#if withMore}
							<button
								type="button"
								class="quiet"
								title="More: another commodity, a settled month"
								aria-label="More for split {n}"
								aria-expanded={more[i] || split.converted || !!split.settles}
								onclick={() => (more[i] = !more[i])}>⋯</button
							>
						{/if}
						<button
							type="button"
							class="quiet"
							disabled={splits.length === 1}
							aria-label="Remove split {n}"
							onclick={() => remove(i)}>✕</button
						>
					</span>
				</div>
				{#if withMore && (more[i] || split.converted || split.settles)}
					<div class="line extra">
						<label class="check">
							<input type="checkbox" bind:checked={split.converted} /> Received a different commodity
						</label>
						{#if split.converted}
							<label class="field">
								<span>Received</span>
								<span class="money">
									<input
										class="qty"
										inputmode="decimal"
										bind:value={split.toQuantity}
										placeholder="0.00"
										aria-label="Split {n}: received"
									/>
									<select bind:value={split.toCommodity} aria-label="Split {n}: received commodity">
										{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
									</select>
								</span>
							</label>
						{/if}
						<label class="field">
							<span>Settles month</span>
							<input
								class="month"
								bind:value={split.settles}
								placeholder="YYYY-MM"
								disabled={!p.settles}
								aria-label="Split {n}: settles month"
							/>
						</label>
						{#if p.kind}
							<span class="kind">{kinds[p.kind]}{p.liquidation ? ', liquidation' : ''}</span>
						{/if}
					</div>
				{/if}
			</li>
		{/each}
	</ol>
</div>
<div class="foot">
	{#if canAdd}
		<button type="button" class="quiet" onclick={add}>Add split</button>
	{:else}
		<span></span>
	{/if}
	{#if expected !== undefined && splits.length > 1 && left !== undefined}
		{@const c = code(splits[0].commodity)}
		<span class="total">
			{#if sign(left) === 0}
				<Mark tone="ok" label="Adds up" /> to the bank's <Amount value={expected} code={c} />
			{:else if sign(left) > 0}
				<Mark tone="error" label="Short" /><Amount value={left} code={c} /> left to split
			{:else}
				<Mark tone="error" label="Over" /><Amount value={left.slice(1)} code={c} /> more than the bank's
				<Amount value={expected} code={c} />
			{/if}
		</span>
	{:else if splits.length > 1 && totals.length}
		<span class="total"
			>Total {#each totals as t, k (k)}{k ? ', ' : ''}<Amount
					value={t.value}
					code={t.code}
				/>{/each}</span
		>
	{/if}
</div>

<style>
	.lines {
		container-type: inline-size;
	}
	.splits {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.split {
		padding: var(--space-2) 0;
		border-top: var(--border-width) solid var(--color-border);
	}
	.heads {
		display: none;
	}
	/* Narrow: each split's fields wrap, each with its label. */
	.line {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-2) var(--space-3);
	}
	.extra {
		margin-top: var(--space-2);
		padding-left: calc(var(--space-2) + 1ch);
	}
	.n {
		align-self: center;
		min-width: 1ch;
		color: var(--color-text-muted);
		font-variant-numeric: var(--numeric);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.field > span:first-child,
	.field > label:first-child,
	.heads,
	.check,
	.kind {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.side select,
	.side input {
		width: 11rem;
	}
	.pick {
		width: 9rem;
	}
	.grow {
		flex: 1;
		min-width: 6rem;
	}
	.arrow {
		padding-bottom: var(--space-2);
		color: var(--color-text-muted);
	}
	.money {
		display: flex;
		gap: var(--space-1);
	}
	.qty {
		width: 7rem;
		text-align: right;
		font-variant-numeric: var(--numeric);
	}
	.month {
		width: 7rem;
	}
	.check {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding-bottom: var(--space-2);
	}
	.kind {
		padding-bottom: var(--space-2);
	}
	.tools {
		display: flex;
		gap: var(--space-1);
	}
	/* The headings' last cell, as wide as a line's two buttons. */
	.tools-space {
		width: calc(2 * 2.75rem + var(--space-1));
	}
	.tools button {
		width: 2.75rem;
		padding-inline: 0;
	}
	/* Wide: a grid, the headings once, a counterparty under its side. */
	@container (min-width: 66rem) {
		.heads,
		.line {
			display: grid;
			grid-template-columns: 1ch 11rem 1rem 11rem 11.5rem 9rem 9rem minmax(4rem, 1fr) max-content;
			gap: var(--space-2);
			align-items: start;
		}
		.heads {
			padding-bottom: var(--space-1);
		}
		.line > .field > span:first-child,
		.line > .field > label:first-child {
			position: absolute;
			width: 1px;
			height: 1px;
			overflow: hidden;
			clip-path: inset(50%);
			white-space: nowrap;
		}
		.n,
		.arrow {
			align-self: start;
			padding: var(--space-2) 0 0;
		}
		.side {
			gap: var(--space-1);
		}
		.money {
			width: 100%;
		}
		.qty {
			flex: 1;
			width: auto;
			min-width: 0;
		}
	}
	/* Where a field's value came from: one line, always there. */
	.status {
		min-height: calc(var(--text-xs) * var(--leading));
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--text-xs);
		color: var(--color-text-muted);
	}
	.status > :global(* + *) {
		margin-left: var(--space-1);
	}
	.fixed {
		display: block;
		padding: var(--space-2) 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.side {
		min-width: 0;
	}
	.foot {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-3);
		padding-top: var(--space-2);
		border-top: var(--border-width) solid var(--color-border);
	}
	.total {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
</style>
