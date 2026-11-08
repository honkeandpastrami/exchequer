<script lang="ts">
	// What committing an import did (docs/ux.md §5): a block for the commit, and
	// one per balance check, each with its colour, one item per line.
	import { resolve } from '$app/paths';
	import type { Schemas } from '../api/client';
	import { accountLabel } from '../names';
	import type { Reference } from '../reference';
	import Amount from './Amount.svelte';
	import Mark from './Mark.svelte';

	let { committed: c, ref }: { committed: Schemas['Committed']; ref: Reference } = $props();

	const several = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
</script>

<section class="block ok">
	<div class="head"><Mark tone="ok" label="Ready" quiet /> Committed</div>
	<dl class="facts">
		<dt>Added</dt>
		<dd>{several(c.added, 'transaction', 'transactions')}</dd>
		<dd class="aside">
			{#if c.import_batch_id}<a
					class="go"
					href={resolve(`/transactions?import_batch_id=${c.import_batch_id}`)}>See them ▸</a
				>{/if}
		</dd>
		<dt>Not added</dt>
		<dd>{several(c.not_added, 'row', 'rows')}</dd>
		<dd class="aside"></dd>
	</dl>
</section>
{#each c.balance_checks as b (b.id)}
	{@const a = ref.accounts.get(b.account_id)}
	{@const code = ref.commodities.get(b.commodity_id)?.code}
	<section class="block" class:ok={b.matches} class:warning={!b.matches}>
		<div class="head">
			{#if b.matches}<Mark tone="ok" label="Matches" quiet />{:else}<Mark
					tone="warning"
					label="Doesn't match"
					quiet
				/>{/if}
			{a ? accountLabel(a, ref) : '?'}
		</div>
		<dl class="facts">
			<dt>Bank</dt>
			<dd><Amount value={b.amount} {code} /> on {b.date}</dd>
			<dd class="aside"></dd>
			<dt>Ledger</dt>
			<dd><Amount value={b.ledger} {code} /></dd>
			<dd class="aside"></dd>
			{#if !b.matches}
				<dt>Off by</dt>
				<dd><Amount value={b.difference} {code} /></dd>
				<dd class="aside">
					<a class="go" href={resolve(`/checks?account=${b.account_id}`)}>Look into it ▸</a>
				</dd>
			{/if}
		</dl>
	</section>
{/each}

<style>
	.block {
		margin-bottom: var(--space-3);
		padding: var(--space-3) var(--space-4);
		background: var(--color-surface);
		border: var(--border-width) solid var(--color-border);
		border-left: 4px solid var(--color-positive);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-1);
	}
	.warning {
		border-left-color: var(--color-warning);
	}
	.head {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		margin-bottom: var(--space-2);
		font-weight: var(--weight-strong);
		overflow-wrap: anywhere;
	}
	.facts {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr) max-content;
		gap: var(--space-1) var(--space-3);
		align-items: baseline;
		margin: 0;
		font-size: var(--text-sm);
	}
	dt {
		color: var(--color-text-muted);
	}
	dd {
		margin: 0;
		min-width: 0;
		overflow-wrap: anywhere;
	}
	.aside {
		justify-self: end;
	}
	.go {
		display: inline-flex;
		align-items: center;
		white-space: nowrap;
	}
</style>
