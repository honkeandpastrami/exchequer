//! A software passkey: what a browser and Face ID would answer to the
//! server's ceremony options. One P-256 key, attestation "none", user
//! verified, and backed up (like an iCloud Keychain passkey).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ciborium::Value as Cbor;
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// User present, user verified, backup eligible, backed up.
const FLAGS: u8 = 0x01 | 0x04 | 0x08 | 0x10;
/// Attested credential data follows.
const ATTESTED: u8 = 0x40;

pub struct Authenticator {
    key: EcdsaKeyPair,
    credential_id: Vec<u8>,
    user_handle: Option<String>,
    counter: u32,
    /// The origin the "browser" reports.
    pub origin: String,
}

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn cbor(v: &Cbor) -> Vec<u8> {
    let mut out = vec![];
    ciborium::into_writer(v, &mut out).unwrap();
    out
}

impl Authenticator {
    /// A fresh passkey; `seed` tells their credential IDs apart.
    pub fn new(seed: u8) -> Self {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        Self {
            key: EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
                .unwrap(),
            credential_id: vec![seed; 16],
            user_handle: None,
            counter: 0,
            origin: super::ORIGIN.to_owned(),
        }
    }

    fn client_data(&self, kind: &str, options: &Value) -> String {
        json!({
            "type": kind,
            "challenge": options["publicKey"]["challenge"],
            "origin": self.origin,
            "crossOrigin": false,
        })
        .to_string()
    }

    /// `navigator.credentials.create(options)`, as JSON.
    pub fn create(&mut self, options: &Value) -> Value {
        let o = &options["publicKey"];
        assert_eq!(o["authenticatorSelection"]["residentKey"], "required");
        self.user_handle = Some(o["user"]["id"].as_str().unwrap().to_owned());
        let rp_id = o["rp"]["id"].as_str().unwrap();
        // Uncompressed: 0x04, then x and y.
        let point = self.key.public_key().as_ref();
        let cose = cbor(&Cbor::Map(vec![
            (1.into(), 2.into()),    // kty: EC2
            (3.into(), (-7).into()), // alg: ES256
            ((-1).into(), 1.into()), // crv: P-256
            ((-2).into(), Cbor::Bytes(point[1..33].to_vec())),
            ((-3).into(), Cbor::Bytes(point[33..].to_vec())),
        ]));
        let mut auth_data = Sha256::digest(rp_id).to_vec();
        auth_data.push(FLAGS | ATTESTED);
        auth_data.extend(self.counter.to_be_bytes());
        auth_data.extend([0u8; 16]); // AAGUID
        auth_data.extend(
            u16::try_from(self.credential_id.len())
                .unwrap()
                .to_be_bytes(),
        );
        auth_data.extend(&self.credential_id);
        auth_data.extend(cose);
        let attestation = cbor(&Cbor::Map(vec![
            ("fmt".into(), "none".into()),
            ("attStmt".into(), Cbor::Map(vec![])),
            ("authData".into(), Cbor::Bytes(auth_data)),
        ]));
        json!({
            "id": b64(&self.credential_id),
            "rawId": b64(&self.credential_id),
            "type": "public-key",
            "response": {
                "attestationObject": b64(&attestation),
                "clientDataJSON": b64(self.client_data("webauthn.create", options).as_bytes()),
            },
            "extensions": {},
        })
    }

    /// `navigator.credentials.get(options)`, as JSON: usernameless, so it
    /// returns the user handle it was registered with.
    pub fn get(&mut self, options: &Value) -> Value {
        let rp_id = options["publicKey"]["rpId"].as_str().unwrap();
        self.counter += 1;
        let mut auth_data = Sha256::digest(rp_id).to_vec();
        auth_data.push(FLAGS);
        auth_data.extend(self.counter.to_be_bytes());
        let client_data = self.client_data("webauthn.get", options);
        let mut signed = auth_data.clone();
        signed.extend(Sha256::digest(client_data.as_bytes()));
        let signature = self.key.sign(&SystemRandom::new(), &signed).unwrap();
        json!({
            "id": b64(&self.credential_id),
            "rawId": b64(&self.credential_id),
            "type": "public-key",
            "response": {
                "authenticatorData": b64(&auth_data),
                "clientDataJSON": b64(client_data.as_bytes()),
                "signature": b64(signature.as_ref()),
                "userHandle": self.user_handle,
            },
            "extensions": {},
        })
    }
}
