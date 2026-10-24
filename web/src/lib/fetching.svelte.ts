// Prices the server is fetching (design §3.4). An answer with `fetching`
// was given with the prices already stored; fresher ones follow. One
// request waits for the fetches to finish (`GET /prices/fetching?wait=…`),
// then `prices.refetch` is bumped, and pages that value at prices load
// again, keeping what they show until the new answer arrives.
import { api, ok } from '#lib/api/client.ts';

/** Under a proxy's usual minute; the server waits at most 50. */
const WAIT_SECS = 25;
/** Waits that time out in a row before giving up: a fetch takes seconds. */
const MAX_WAITS = 4;
/** Reloads in a row that still say `fetching`: each started a new fetch. */
const MAX_ROUNDS = 3;

export const prices = $state({ refetch: 0 });

let waiting = false;
let rounds = 0;

/** Notes whether `answer` is waiting on prices, and passes it on. */
export function fetched<T extends { fetching: boolean }>(answer: T): T {
	if (!answer.fetching) {
		rounds = 0;
	} else if (!waiting && rounds < MAX_ROUNDS) {
		rounds += 1;
		void wait();
	}
	return answer;
}

async function wait() {
	waiting = true;
	try {
		for (let i = 0; i < MAX_WAITS; i++) {
			const r = ok(
				await api.GET('/api/v1/prices/fetching', { params: { query: { wait: WAIT_SECS } } })
			);
			if (!r.fetching) {
				prices.refetch += 1;
				return;
			}
		}
	} catch {
		// Pages keep what they show; the next load asks again.
	} finally {
		waiting = false;
	}
}

/**
 * `load()`, again whenever what it reads changes or prices may have
 * arrived. `value` is its latest answer for the current `key()`: it stays
 * while the same key reloads, and is `null` until a new key's first answer.
 * Call it while a component initializes.
 */
export function latest<T>(key: () => string, load: () => Promise<T>) {
	const promise = $derived((void prices.refetch, load()));
	let kept: { key: string; value: T } | null = $state(null);
	$effect(() => {
		const p = promise;
		const k = key();
		p.then((value) => {
			if (p === promise) kept = { key: k, value };
		}).catch(() => {});
	});
	return {
		get promise() {
			return promise;
		},
		get value() {
			return kept?.key === key() ? kept.value : null;
		}
	};
}
