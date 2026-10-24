import { describe, expect, it } from 'vitest';
import { creationOptions, deviceName, requestOptions, toBase64url, toBytes } from './webauthn';

const bytes = (b: ArrayBuffer | BufferSource) => [...new Uint8Array(b as ArrayBuffer)];

describe('base64url', () => {
	it('round-trips any bytes, without padding', () => {
		for (const raw of [[], [0], [251, 255], [1, 2, 3, 4, 5], [...Array(256).keys()]]) {
			const text = toBase64url(new Uint8Array(raw).buffer);
			expect(text).not.toMatch(/[+/=]/);
			expect(bytes(toBytes(text))).toEqual(raw);
		}
	});
});

describe('ceremony options', () => {
	it('turn base64url fields into bytes for the browser', () => {
		const create = creationOptions({
			publicKey: {
				challenge: 'AQID',
				rp: { id: 'books.example.org', name: 'Exchequer' },
				user: { id: '-_8', name: 'Alex', displayName: 'Alex' },
				excludeCredentials: [{ type: 'public-key', id: 'BAU' }]
			}
		}).publicKey!;
		expect(bytes(create.challenge)).toEqual([1, 2, 3]);
		expect(bytes(create.user.id)).toEqual([251, 255]);
		expect(create.user.name).toBe('Alex');
		expect(bytes(create.excludeCredentials![0].id)).toEqual([4, 5]);

		const get = requestOptions({
			publicKey: { challenge: 'AQID', rpId: 'books.example.org' }
		}).publicKey!;
		expect(bytes(get.challenge)).toEqual([1, 2, 3]);
		expect(get.allowCredentials).toEqual([]);
		expect(get.rpId).toBe('books.example.org');
	});
});

describe('deviceName', () => {
	it('names the device from its user agent', () => {
		expect(deviceName('Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X)')).toBe('iPhone');
		expect(deviceName('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('Mac');
		expect(deviceName('curl/8')).toBe('Passkey');
	});
});
