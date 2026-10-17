<script lang="ts">
	// One split of the transaction editor. Optional fields appear only when
	// the split can have them (see `preview`).
	import { kinds, partyKinds, rank } from '../labels';
	import { accountLabel } from '../names';
	import { sorted, type Reference } from '../reference';
	import { OUTSIDE, preview, type LegDraft, type SplitDraft } from '../views/editor';

	let {
		split = $bindable(),
		ref,
		title,
		onremove
	}: { split: SplitDraft; ref: Reference; title: string; onremove?: () => void } = $props();

	let p = $derived(preview(split, ref));

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
</script>

{#snippet side(label: string, l: LegDraft)}
	<label class="field">
		<span>{label}</span>
		<select bind:value={l.account}>
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
				list="counterparties"
				placeholder="Counterparty (blank if unknown)"
				aria-label="{label}: counterparty"
			/>
		{/if}
	</label>
{/snippet}

<fieldset class="split">
	<legend>
		{title}
		{#if p.kind}<span class="kind">{kinds[p.kind]}{p.liquidation ? ', liquidation' : ''}</span>{/if}
	</legend>
	<div class="row">
		{@render side('From', split.from)}
		<span class="arrow" aria-hidden="true">→</span>
		{@render side('To', split.to)}
	</div>
	<div class="row">
		<label class="field">
			<span>Amount</span>
			<span class="money">
				<input class="qty" inputmode="decimal" bind:value={split.quantity} placeholder="0.00" />
				<select bind:value={split.commodity} aria-label="Commodity">
					{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
				</select>
			</span>
		</label>
		<label class="check">
			<input type="checkbox" bind:checked={split.converted} /> Received a different commodity
		</label>
		{#if split.converted}
			<label class="field">
				<span>Received</span>
				<span class="money">
					<input class="qty" inputmode="decimal" bind:value={split.toQuantity} placeholder="0.00" />
					<select bind:value={split.toCommodity} aria-label="Received commodity">
						{#each commodities as c (c.id)}<option value={c.id}>{c.code}</option>{/each}
					</select>
				</span>
			</label>
		{/if}
	</div>
	<div class="row">
		{#if p.category}
			<label class="field">
				<span>Category</span>
				<select bind:value={split.category}>
					<option value="">None</option>
					{#each sorted(ref.categories) as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
				</select>
			</label>
		{/if}
		{#if p.purpose}
			<label class="field">
				<span>For</span>
				<!-- No purpose picked: the default party, shown by name like any other. -->
				<select
					value={split.purpose || p.defaultPurpose || ''}
					onchange={(e) => (split.purpose = e.currentTarget.value)}
				>
					{#if !split.purpose && !p.defaultPurpose}<option value="" disabled>Choose…</option>{/if}
					{#each owners as o (o.id)}<option value={o.id}>{o.name}</option>{/each}
				</select>
			</label>
		{/if}
		{#if p.settles}
			<label class="field">
				<span>Settles month</span>
				<input class="month" bind:value={split.settles} placeholder="YYYY-MM" />
			</label>
		{/if}
		<label class="field grow">
			<span>Note</span>
			<input bind:value={split.description} />
		</label>
	</div>
	{#if onremove}
		<button type="button" class="quiet remove" onclick={onremove}>Remove split</button>
	{/if}
</fieldset>

<style>
	.split {
		margin: 0 0 var(--space-4);
		padding: var(--space-3) var(--space-4) var(--space-4);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-md);
		background: var(--color-surface);
	}
	legend {
		padding: 0 var(--space-2);
		font-weight: var(--weight-strong);
	}
	.kind {
		margin-left: var(--space-2);
		font-weight: var(--weight-normal);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		gap: var(--space-3);
		margin-top: var(--space-3);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 12rem;
	}
	.field > span:first-child,
	.check {
		font-size: var(--text-sm);
		color: var(--color-text-muted);
	}
	.field input:not(.qty):not(.month) {
		min-width: 12rem;
	}
	.grow {
		flex: 1;
	}
	.arrow {
		margin-top: calc(var(--text-sm) * var(--leading) + var(--space-1) + var(--space-2));
		color: var(--color-text-muted);
	}
	.money {
		display: flex;
		gap: var(--space-1);
	}
	.qty {
		width: 9rem;
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
		align-self: flex-end;
		padding-bottom: var(--space-2);
	}
	.remove {
		margin-top: var(--space-3);
		font-size: var(--text-sm);
	}
</style>
