<script lang="ts">
	// A transaction's splits, one line each. Fields a split can't have stay in
	// their place, disabled (see `preview`); a conversion's received amount and
	// a settled month go on a second line, under More.
	import { kinds, partyKinds, rank } from '../labels';
	import { accountLabel } from '../names';
	import { sorted, type Reference } from '../reference';
	import {
		blankSplit,
		OUTSIDE,
		preview,
		splitTotals,
		type LegDraft,
		type SplitDraft
	} from '../views/editor';
	import Amount from './Amount.svelte';

	let {
		splits = $bindable(),
		ref,
		commodity
	}: {
		splits: SplitDraft[];
		ref: Reference;
		/** A new split's commodity when there's none to copy. */
		commodity: string;
	} = $props();

	let more: boolean[] = $state([]);
	// Several forms may be open at once.
	const uid = $props.id();

	const owners = $derived(
		[...ref.parties.values()].sort(
			(a, b) => rank(partyKinds, a.kind) - rank(partyKinds, b.kind) || a.name.localeCompare(b.name)
		)
	);
	const accountsOf = (owner: string, keep: string) =>
		sorted(ref.accounts).filter((a) => a.owner_id === owner && (!a.archived || a.id === keep));
	const commodities = $derived(
		[...ref.commodities.values()].sort((a, b) => a.code.localeCompare(b.code))
	);

	const totals = $derived(
		splitTotals(splits).map((t) => ({
			code: ref.commodities.get(t.commodity)?.code,
			value: t.value
		}))
	);

	function add() {
		splits.push(blankSplit(commodity, splits.at(-1)));
	}
	function remove(i: number) {
		splits.splice(i, 1);
		more.splice(i, 1);
	}
</script>

{#snippet side(label: string, l: LegDraft, n: number)}
	<label class="field side">
		<span>{label}</span>
		<select bind:value={l.account} aria-label="Split {n}: {label}">
			<option value="" disabled>Choose…</option>
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
	</label>
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
			{@const p = preview(split, ref)}
			{@const n = i + 1}
			{@const k = p.kind ? kinds[p.kind].toLowerCase() : ''}
			{@const notFor = k
				? `Not for ${/^[aeiou]/.test(k) ? 'an' : 'a'} ${k}`
				: 'Choose both sides first'}
			<li class="split">
				<div class="line">
					<span class="n" aria-hidden="true">{n}</span>
					{@render side('From', split.from, n)}
					<span class="arrow" aria-hidden="true">→</span>
					{@render side('To', split.to, n)}
					<label class="field">
						<span>Amount</span>
						<span class="money">
							<input
								class="qty"
								inputmode="decimal"
								bind:value={split.quantity}
								placeholder="0.00"
								aria-label="Split {n}: amount"
							/>
							<select bind:value={split.commodity} aria-label="Split {n}: commodity">
								{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
							</select>
						</span>
					</label>
					<label class="field">
						<span>Category</span>
						{#if p.category}
							<select class="pick" bind:value={split.category} aria-label="Split {n}: category">
								<option value="">None</option>
								{#each sorted(ref.categories) as c (c.id)}<option value={c.id}>{c.name}</option
									>{/each}
							</select>
						{:else}
							<select class="pick" disabled title={notFor} aria-label="Split {n}: category"
								><option>Doesn't apply</option></select
							>
						{/if}
					</label>
					<label class="field">
						<span>For</span>
						{#if p.purpose}
							<!-- No purpose picked: the default party, shown by name like any other. -->
							<select
								class="pick"
								value={split.purpose || p.defaultPurpose || ''}
								onchange={(e) => (split.purpose = e.currentTarget.value)}
								aria-label="Split {n}: for"
							>
								{#if !split.purpose && !p.defaultPurpose}<option value="" disabled>Choose…</option
									>{/if}
								{#each owners as o (o.id)}<option value={o.id}>{o.name}</option>{/each}
							</select>
						{:else}
							<select class="pick" disabled title={notFor} aria-label="Split {n}: for"
								><option>Doesn't apply</option></select
							>
						{/if}
					</label>
					<label class="field grow">
						<span>Note</span>
						<input bind:value={split.description} aria-label="Split {n}: note" />
					</label>
					<span class="tools">
						<button
							type="button"
							class="quiet"
							title="More: another commodity, a settled month"
							aria-label="More for split {n}"
							aria-expanded={more[i] || split.converted || !!split.settles}
							onclick={() => (more[i] = !more[i])}>⋯</button
						>
						<button
							type="button"
							class="quiet"
							disabled={splits.length === 1}
							aria-label="Remove split {n}"
							onclick={() => remove(i)}>✕</button
						>
					</span>
				</div>
				{#if more[i] || split.converted || split.settles}
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
	<button type="button" class="quiet" onclick={add}>Add split</button>
	{#if splits.length > 1 && totals.length}
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
		.line > .field > span:first-child {
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
