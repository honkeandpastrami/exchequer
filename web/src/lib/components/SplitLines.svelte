<script lang="ts" module>
	/** A field a bank row's label can be for. */
	export type Field = 'other' | 'category' | 'purpose';
</script>

<script lang="ts">
	// A transaction's splits (ux.md §10): one field per line, in the order
	// from, to, amount, category, For, note; with two or more, each split is
	// a numbered block. Fields a split can't have stay in their place,
	// disabled (see `preview`); the rarer ones (another commodity received, a
	// settled month) open under More fields. Staged bank rows use it too:
	// their account's side is fixed, the amount is the bank's until split,
	// and each field may have a badge and a line saying where its value came
	// from.
	import type { Snippet } from 'svelte';
	import { kinds, partyKinds, rank } from '../labels';
	import { minus, sign } from '../money';
	import { accountLabel } from '../names';
	import { sorted, type Reference } from '../reference';
	import {
		blankSplit,
		cleanQuantity,
		OUTSIDE,
		preview,
		splitTotals,
		UNSET,
		type LegDraft,
		type SplitDraft
	} from '../views/editor';
	import Amount from './Amount.svelte';
	import FieldLine from './Field.svelte';
	import Mark from './Mark.svelte';
	import SidePicker from './SidePicker.svelte';

	let {
		splits = $bindable(),
		ref,
		commodity,
		fixed,
		expected,
		canAdd = true,
		more: withMore = true,
		badge,
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
		/** Whether More fields (another commodity, a settled month) is offered. */
		more?: boolean;
		/** Split `i`'s field's badge: how sure its suggestion is. */
		badge?: Snippet<[number, Field]>;
		/** A line under split `i`'s field: where its value came from. */
		status?: Snippet<[number, Field]>;
	} = $props();

	let more: boolean[] = $state([]);
	// Several forms may be open at once.
	const uid = $props.id();

	const owners = $derived(
		[...ref.parties.values()].sort(
			(a, b) => rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)
		)
	);
	const commodities = $derived(
		[...ref.commodities.values()].sort((a, b) => a.code.localeCompare(b.code))
	);
	const code = (id: string) => ref.commodities.get(id)?.code;

	const totals = $derived(
		splitTotals(splits).map((t) => ({ code: code(t.commodity), value: t.value }))
	);
	/** Every split's amount typed. */
	const typed = $derived(splits.every((s) => /^\d+(\.\d+)?$/.test(cleanQuantity(s.quantity))));
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
	{@const id = `${uid}-${i}-${label}`}
	{#snippet sideBadge()}{@render badge?.(i, 'other')}{/snippet}
	{#snippet sideStatus()}{@render status?.(i, 'other')}{/snippet}
	{#if fixed && l !== otherOf(s)}
		{@const a = ref.accounts.get(fixed.account)}
		<FieldLine {label}><span class="fixed">{a ? accountLabel(a, ref) : '?'}</span></FieldLine>
	{:else}
		<FieldLine {label} {id} badge={sideBadge} status={status ? sideStatus : undefined}>
			<SidePicker
				leg={l}
				set={(v) => Object.assign(l, v)}
				{ref}
				{id}
				label="Split {n}: {label}"
				skip={fixed?.account}
			/>
		</FieldLine>
	{/if}
{/snippet}

<div class="splits">
	{#each splits as split, i (i)}
		{@const p = previewOf(split)}
		{@const n = i + 1}
		{@const k = p.kind ? kinds[p.kind].toLowerCase() : ''}
		{@const notFor = k
			? `Not for ${/^[aeiou]/.test(k) ? 'an' : 'a'} ${k}`
			: 'Choose both sides first'}
		{@const whole = expected !== undefined && splits.length === 1}
		{#snippet categoryBadge()}{#if p.category}{@render badge?.(i, 'category')}{/if}{/snippet}
		{#snippet categoryStatus()}{@render status?.(i, 'category')}{/snippet}
		{#snippet purposeBadge()}{#if p.purpose}{@render badge?.(i, 'purpose')}{/if}{/snippet}
		{#snippet purposeStatus()}{@render status?.(i, 'purpose')}{/snippet}
		<section class="split" aria-label={splits.length > 1 ? `Split ${n}` : undefined}>
			{#if splits.length > 1}<h4 class="head">Split {n}</h4>{/if}
			{@render side('From', split, split.from, i)}
			{@render side('To', split, split.to, i)}
			<FieldLine label="Amount" id="{uid}-{i}-amount">
				<span class="money">
					<input
						id="{uid}-{i}-amount"
						class="qty"
						inputmode="decimal"
						bind:value={split.quantity}
						placeholder="0.00"
						disabled={whole}
						aria-label="Split {n}: amount"
					/>
					<select bind:value={split.commodity} disabled={!!fixed} aria-label="Split {n}: commodity">
						{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
					</select>
				</span>
			</FieldLine>
			<FieldLine
				label="Category"
				id="{uid}-{i}-category"
				badge={categoryBadge}
				status={p.category && status ? categoryStatus : undefined}
			>
				{#if p.category}
					<!-- Required where a purpose is (design §4.1). -->
					{@const unset = split.category === UNSET || (p.purpose && !split.category)}
					<select
						id="{uid}-{i}-category"
						value={unset ? UNSET : split.category}
						onchange={(e) => (split.category = e.currentTarget.value)}
						aria-label="Split {n}: category"
					>
						{#if unset}<option value={UNSET} disabled>Choose…</option>{/if}
						{#if !p.purpose}<option value="">None</option>{/if}
						{#each sorted(ref.categories) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
					</select>
				{:else}
					<select id="{uid}-{i}-category" disabled title={notFor} aria-label="Split {n}: category"
						><option>Doesn't apply</option></select
					>
				{/if}
			</FieldLine>
			<FieldLine
				label="For"
				id="{uid}-{i}-purpose"
				badge={purposeBadge}
				status={p.purpose && status ? purposeStatus : undefined}
			>
				{#if p.purpose}
					{@const unset = !split.purpose || split.purpose === UNSET}
					<select
						id="{uid}-{i}-purpose"
						value={unset ? UNSET : split.purpose}
						onchange={(e) => (split.purpose = e.currentTarget.value)}
						aria-label="Split {n}: for"
					>
						{#if unset}<option value={UNSET} disabled>Choose…</option>{/if}
						{#each owners as o (o.id)}<option value={o.id}>{o.name}</option>{/each}
					</select>
				{:else}
					<select id="{uid}-{i}-purpose" disabled title={notFor} aria-label="Split {n}: for"
						><option>Doesn't apply</option></select
					>
				{/if}
			</FieldLine>
			<FieldLine label="Note" id="{uid}-{i}-note">
				<input id="{uid}-{i}-note" bind:value={split.description} aria-label="Split {n}: note" />
			</FieldLine>
			{#if withMore && (more[i] || split.converted || split.settles)}
				<FieldLine label="Received">
					<label class="choice">
						<input type="checkbox" bind:checked={split.converted} /> A different commodity
					</label>
				</FieldLine>
				{#if split.converted}
					<FieldLine label="Amount received" id="{uid}-{i}-received">
						<span class="money">
							<input
								id="{uid}-{i}-received"
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
					</FieldLine>
				{/if}
				<FieldLine label="Settles month" id="{uid}-{i}-settles">
					<input
						id="{uid}-{i}-settles"
						bind:value={split.settles}
						placeholder="YYYY-MM"
						disabled={!p.settles}
						aria-label="Split {n}: settles month"
					/>
				</FieldLine>
				{#if p.kind}<FieldLine label="Kind"><span class="fixed">{kinds[p.kind]}</span></FieldLine
					>{/if}
			{/if}
			{#if (withMore && !split.converted && !split.settles) || splits.length > 1}
				<div class="tools">
					{#if withMore && !split.converted && !split.settles}
						<button
							type="button"
							class="quiet"
							aria-expanded={!!more[i]}
							onclick={() => (more[i] = !more[i])}
							>{more[i] ? 'Fewer fields' : 'More fields'}</button
						>
					{/if}
					{#if splits.length > 1}
						<button type="button" class="quiet" onclick={() => remove(i)}>Remove split</button>
					{/if}
				</div>
			{/if}
		</section>
	{/each}
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
			{#if !typed}
				<Mark tone="open" label="Incomplete" /> every split needs an amount
			{:else if sign(left) === 0}
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
	.split {
		padding: var(--space-2) 0;
		border-top: var(--border-width) solid var(--color-border);
	}
	.head {
		margin: 0 0 var(--space-1);
		font-family: var(--font-ui);
		font-size: var(--text-sm);
	}
	.money {
		display: flex;
		gap: var(--space-1);
	}
	.qty {
		flex: 1;
		min-width: 0;
		text-align: right;
		font-variant-numeric: var(--numeric);
	}
	.fixed {
		display: block;
		padding: var(--space-2) 0;
		overflow-wrap: anywhere;
	}
	.tools {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
		padding-top: var(--space-1);
	}
	.foot {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
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
