<script lang="ts">
	// One side of a split: an account with its owner muted before it, a
	// counterparty, or (muted) an unknown outside party.
	import type { Schemas } from '../api/client';
	import type { Reference } from '../reference';
	import { legName } from '../views/transactions';

	let { leg, ref }: { leg: Schemas['Leg']; ref: Reference } = $props();

	let account = $derived(leg.account_id ? ref.accounts.get(leg.account_id) : undefined);
</script>

{#if account}<span class="owner">{ref.parties.get(account.owner_id)?.name ?? '?'} ·</span>
	{account.name}{:else if !leg.account_id && !leg.counterparty_id}<span class="unknown"
		>{legName(leg, ref)}</span
	>{:else}{legName(leg, ref)}{/if}

<style>
	.owner,
	.unknown {
		color: var(--color-text-muted);
	}
</style>
