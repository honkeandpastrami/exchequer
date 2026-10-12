// Display names for API enums, in display order.

import type { Schemas } from './api/client';

export const accountKinds: Record<Schemas['AccountKind'], string> = {
	checking: 'Checking',
	savings: 'Savings',
	cash: 'Cash',
	credit_card: 'Credit card',
	mortgage: 'Mortgage',
	brokerage: 'Brokerage',
	other: 'Other'
};

export const partyKinds: Record<Schemas['PartyKind'], string> = {
	household: 'Household',
	person: 'Person',
	business: 'Business'
};

/** Position of `key` in `labels`' order, for sorting. */
export function rank<K extends string>(labels: Record<K, string>, key: K): number {
	return Object.keys(labels).indexOf(key);
}

export const kinds: Record<Schemas['Kind'], string> = {
	expense: 'Expense',
	income: 'Income',
	transfer: 'Transfer',
	conversion: 'Conversion',
	adjustment: 'Adjustment'
};

export const lineKinds: Record<Schemas['LineKind'], string> = {
	claim: 'Claim',
	payment: 'Payment',
	in_transit: 'In transit',
	clearing: 'Clearing'
};

/** Why the discrepancy finder suggests a transaction (design §7.1). */
export const candidateReasons: Record<Schemas['CandidateReason'], string> = {
	duplicate_pair: 'Recorded twice',
	equals_difference: 'Equal to the difference',
	date_boundary: 'Dated next to the check date',
	sign_flipped: 'Recorded with the wrong sign',
	subset: 'Together equal to the difference',
	transposition: 'Two digits swapped',
	near_duplicate: 'Same amount twice in a few days',
	multiple_of_nine: 'The difference is a multiple of 9, the mark of swapped digits'
};
