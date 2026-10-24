// Passkeys (design §10): the server's ceremony options carry binary fields as
// base64url strings; `navigator.credentials` wants ArrayBuffers, and its
// answer goes back to the server as JSON the same way.

type Json = Record<string, unknown>;

export function toBytes(base64url: string): ArrayBuffer {
	const base64 = base64url.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(base64.padEnd(base64.length + ((4 - (base64.length % 4)) % 4), '='));
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes.buffer;
}

export function toBase64url(buffer: ArrayBuffer): string {
	let binary = '';
	for (const b of new Uint8Array(buffer)) binary += String.fromCharCode(b);
	return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** The server's `{ publicKey }` for `navigator.credentials.create`. */
export function creationOptions(options: Json): CredentialCreationOptions {
	const o = options.publicKey as Json;
	const user = o.user as Json;
	return {
		publicKey: {
			...(o as unknown as PublicKeyCredentialCreationOptions),
			challenge: toBytes(o.challenge as string),
			user: {
				...(user as unknown as PublicKeyCredentialUserEntity),
				id: toBytes(user.id as string)
			},
			excludeCredentials: ((o.excludeCredentials as Json[] | undefined) ?? []).map((c) => ({
				...(c as unknown as PublicKeyCredentialDescriptor),
				id: toBytes(c.id as string)
			}))
		}
	};
}

/** The server's `{ publicKey }` for `navigator.credentials.get`. */
export function requestOptions(options: Json): CredentialRequestOptions {
	const o = options.publicKey as Json;
	return {
		publicKey: {
			...(o as unknown as PublicKeyCredentialRequestOptions),
			challenge: toBytes(o.challenge as string),
			allowCredentials: ((o.allowCredentials as Json[] | undefined) ?? []).map((c) => ({
				...(c as unknown as PublicKeyCredentialDescriptor),
				id: toBytes(c.id as string)
			}))
		}
	};
}

/** The browser's answer, as the server reads it. */
export function credentialJson(c: PublicKeyCredential): Json {
	const r = c.response;
	const response: Json = { clientDataJSON: toBase64url(r.clientDataJSON) };
	if (r instanceof AuthenticatorAttestationResponse) {
		response.attestationObject = toBase64url(r.attestationObject);
		response.transports = r.getTransports?.() ?? [];
	} else if (r instanceof AuthenticatorAssertionResponse) {
		response.authenticatorData = toBase64url(r.authenticatorData);
		response.signature = toBase64url(r.signature);
		response.userHandle = r.userHandle ? toBase64url(r.userHandle) : null;
	}
	return {
		id: c.id,
		rawId: toBase64url(c.rawId),
		type: c.type,
		response,
		extensions: c.getClientExtensionResults()
	};
}

/** What to call a passkey made on this device: `iPhone`, `Mac`… */
export function deviceName(userAgent: string): string {
	for (const [needle, name] of [
		['iPhone', 'iPhone'],
		['iPad', 'iPad'],
		['Android', 'Android'],
		['Macintosh', 'Mac'],
		['Windows', 'Windows'],
		['Linux', 'Linux']
	] as const) {
		if (userAgent.includes(needle)) return name;
	}
	return 'Passkey';
}
