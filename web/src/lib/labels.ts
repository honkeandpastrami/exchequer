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
	conversion: 'Conversion'
};

export const lineKinds: Record<Schemas['LineKind'], string> = {
	claim: 'Claim',
	payment: 'Payment',
	in_transit: 'In transit',
	clearing: 'Clearing'
};
