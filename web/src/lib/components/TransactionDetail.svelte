<script lang="ts">
	import type { Schemas } from '../api/client';
	import { kinds } from '../labels';
	import type { Reference } from '../reference';
	import LegName from './LegName.svelte';
	import Amount from './Amount.svelte';

	let { tx, ref }: { tx: Schemas['Transaction']; ref: Reference } = $props();

	const code = (leg: Schemas['Leg']) => ref.commodities.get(leg.commodity_id)?.code ?? '?';
</script>

<div class="detail">
	<table>
		<thead>
			<tr>
				<th>From</th>
				<th>To</th>
				<th class="num">Amount</th>
				<th>Kind</th>
				<th>Category</th>
				<th>Purpose</th>
				<th>Note</th>
			</tr>
		</thead>
		<tbody>
			{#each tx.splits as s, i (i)}
				<tr>
					<td><LegName leg={s.from} {ref} /></td>
					<td><LegName leg={s.to} {ref} /></td>
					<td class="num">
						<Amount value={s.from.quantity} code={code(s.from)} />
						{#if s.from.commodity_id !== s.to.commodity_id}
							→ <Amount value={s.to.quantity} code={code(s.to)} />
						{/if}
					</td>
					<td>
						{kinds[s.kind]}
						{#if s.liquidation}<span class="muted">(liquidation)</span>{/if}
					</td>
					<td>{s.category_id ? (ref.categories.get(s.category_id)?.name ?? '?') : ''}</td>
					<td>
						{s.purpose_id ? (ref.parties.get(s.purpose_id)?.name ?? '?') : ''}
						{#if s.settles}<span class="muted">settles {s.settles}</span>{/if}
					</td>
					<td class="muted">{s.description ?? ''}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p class="muted meta">
		{#each tx.external_refs as e (e.source)}<span>{e.source} {e.ref}</span>{/each}
		<span>id {tx.id}</span>
	</p>
</div>

<style>
	.detail {
		padding: var(--space-2) var(--space-3) var(--space-3);
		background: var(--color-surface-alt);
		border-radius: var(--radius-sm);
	}
	.meta {
		display: flex;
		gap: var(--space-4);
		margin: var(--space-2) 0 0;
		font-size: var(--text-xs);
		font-family: var(--font-mono);
	}
</style>
