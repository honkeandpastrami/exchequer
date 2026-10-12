// Import batches (design §6.7): every transaction belongs to the one it
// entered with.

import type { Schemas } from '../api/client';

const kindNames: Record<Schemas['BatchKind'], string> = {
	manual: 'Entered by hand',
	migration: 'Migration',
	file: 'File import'
};

/** What to call a batch: its label, else what kind it is. */
export function batchName(b: Schemas['ImportBatch']): string {
	return b.label ?? kindNames[b.kind];
}
