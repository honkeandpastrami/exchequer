// The bearer token, kept in this browser's localStorage until signing out:
// a passkey session's (design §10), or an API token typed in.

import { forgetReference } from './reference';

const KEY = 'exchequer.token';

function stored(): string | null {
	try {
		return localStorage.getItem(KEY);
	} catch {
		return null;
	}
}

export const session = $state({ token: stored() });

export function signIn(token: string) {
	localStorage.setItem(KEY, token);
	session.token = token;
}

export function signOut() {
	localStorage.removeItem(KEY);
	session.token = null;
	forgetReference();
}
