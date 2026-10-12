// The setup pages edit reference data: each change reloads it for every page.

import { forgetReference, reference, type Reference } from './reference';

export function referenceState() {
	const s = $state<{ ref: Reference | null; failure: unknown }>({ ref: null, failure: null });
	async function reload(forget = true) {
		if (forget) forgetReference();
		try {
			s.ref = await reference();
			s.failure = null;
		} catch (e) {
			s.failure = e;
		}
	}
	reload(false);
	return { state: s, reload: () => reload() };
}
