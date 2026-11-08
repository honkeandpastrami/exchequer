// Settlement, the household pool and defaults for new amounts are in USD.

import type { Reference } from './reference';

export const SETTLEMENT_CODE = 'USD';

/** The USD commodity, once it exists. */
export const usd = (ref: Reference) =>
	[...ref.commodities.values()].find((c) => c.code === SETTLEMENT_CODE);
