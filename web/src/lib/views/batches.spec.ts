import { describe, expect, it } from 'vitest';
import { batchName } from './batches';

const batch = (kind: 'manual' | 'migration' | 'file', label?: string) => ({
	id: 'b1',
	kind,
	label,
	created_at: '2026-06-01T00:00:00Z',
	transaction_count: 1
});

describe('import batches', () => {
	it('are named by their label, else their kind', () => {
		expect(batchName(batch('migration', 'From the old app'))).toBe('From the old app');
		expect(batchName(batch('manual'))).toBe('Entered by hand');
		expect(batchName(batch('file'))).toBe('File import');
	});
});
