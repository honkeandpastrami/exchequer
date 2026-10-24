// Prices the server is fetching (design §3.4). An answer with `fetching`
// was given with the prices already stored; fresher ones may follow. While
// answers say so, `prices.refetch` is bumped every few seconds, and pages
// that value at prices load again, keeping what they show until then.

const WAIT_MS = 2000;
/** Reloads in a row before giving up: a fetch takes seconds, not a minute. */
const MAX_TRIES = 15;

export const prices = $state({ refetch: 0 });

let tries = 0;
let timer: ReturnType<typeof setTimeout> | undefined;

/** Notes whether `answer` is waiting on prices, and passes it on. */
export function fetched<T extends { fetching: boolean }>(answer: T): T {
	if (!answer.fetching) {
		tries = 0;
	} else if (timer === undefined && tries < MAX_TRIES) {
		tries += 1;
		timer = setTimeout(() => {
			timer = undefined;
			prices.refetch += 1;
		}, WAIT_MS);
	}
	return answer;
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
