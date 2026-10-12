// Accounts as people see them: with their owner.

import type { Schemas } from './api/client';
import { partyKinds, rank } from './labels';
import type { Reference } from './reference';

/** An account shown on its own, with its owner: `Alex · Checking`. */
export function accountLabel(a: Schemas['Account'], ref: Reference): string {
	return `${ref.parties.get(a.owner_id)?.name ?? '?'} · ${a.name}`;
}

/** Accounts by owner (household, people, businesses), then by name, for pickers. */
export function accountsByOwner(ref: Reference): Schemas['Account'][] {
	const owner = (a: Schemas['Account']) => ref.parties.get(a.owner_id);
	return [...ref.accounts.values()].sort((a, b) => {
		const x = owner(a);
		const y = owner(b);
		return (
			rank(partyKinds, x?.kind ?? 'business') - rank(partyKinds, y?.kind ?? 'business') ||
			(x?.name ?? '').localeCompare(y?.name ?? '') ||
			a.name.localeCompare(b.name)
		);
	});
}
