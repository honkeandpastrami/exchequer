// Signing in with a passkey, and registering one (design §10).

import { api, ok } from './api/client';
import { signIn } from './session.svelte';
import { creationOptions, credentialJson, deviceName, requestOptions } from './webauthn';

type Json = Record<string, unknown>;

/** The browser's error, said plainly. */
function cancelled(e: unknown): never {
	if (e instanceof DOMException && e.name === 'NotAllowedError')
		throw new Error('Cancelled, or no passkey for this site on this device.');
	throw e;
}

/** Registers a passkey on this device with an invite, and signs in as its user. */
export async function register(invite: string): Promise<void> {
	const start = ok(await api.POST('/api/v1/auth/register', { body: { invite } }));
	const credential = (await navigator.credentials
		.create(creationOptions(start.options as Json))
		.catch(cancelled)) as PublicKeyCredential;
	const session = ok(
		await api.POST('/api/v1/auth/register/{id}', {
			params: { path: { id: start.id } },
			body: { credential: credentialJson(credential), name: deviceName(navigator.userAgent) }
		})
	);
	signIn(session.token);
}

/** Signs in with a passkey this device holds for the server. */
export async function signInWithPasskey(): Promise<void> {
	const start = ok(await api.POST('/api/v1/auth/sign-in'));
	const credential = (await navigator.credentials
		.get(requestOptions(start.options as Json))
		.catch(cancelled)) as PublicKeyCredential;
	const session = ok(
		await api.POST('/api/v1/auth/sign-in/{id}', {
			params: { path: { id: start.id } },
			body: { credential: credentialJson(credential) }
		})
	);
	signIn(session.token);
}
