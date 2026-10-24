//! Bearer-token authentication (design §10).
//!
//! A token looks like `exq_<id>_<secret>`: 32 hex digits of token ID, then 64
//! hex digits (256 bits) of random secret. The server stores only the SHA-256
//! of the secret, finds the token by its ID, and compares hashes in constant
//! time. Invites (`exi_`) and the CLI's device codes (`exd_`) are made and
//! checked the same way.
//!
//! SHA-256 rather than a password hash (argon2, bcrypt) on purpose: those are
//! deliberately slow to protect *guessable* passwords. A 256-bit random secret
//! can't be guessed, so a fast hash is just as safe, and costs microseconds
//! per request instead of milliseconds.
//!
//! Everything under `/auth` is how a token is obtained, so it needs none. The
//! bootstrap token (a podman secret) is accepted only on `/users` and
//! `/tokens`, to add the first user and the first tokens.

use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use domain::TokenId;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::AppState;
use crate::problem::Problem;

/// Who is making the request; available to handlers as an extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Principal {
    Token(TokenId),
    Bootstrap,
}

impl Principal {
    /// The token recorded in the audit log (`None` for bootstrap).
    #[must_use]
    pub const fn actor(self) -> Option<TokenId> {
        match self {
            Self::Token(id) => Some(id),
            Self::Bootstrap => None,
        }
    }
}

/// What a secret is for: its prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Token,
    Invite,
    Device,
}

impl Kind {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Token => "exq_",
            Self::Invite => "exi_",
            Self::Device => "exd_",
        }
    }
}

/// A freshly generated secret. `text` is shown exactly once; the database
/// keeps `hash`.
pub struct NewSecret {
    pub id: Uuid,
    pub text: String,
    pub hash: String,
}

/// Generates a secret of this kind under a fresh ID.
pub fn generate(kind: Kind) -> Result<NewSecret, Problem> {
    let id = Uuid::now_v7();
    let secret = hex(&random::<32>()?);
    Ok(NewSecret {
        id,
        hash: hash(&secret),
        text: format!("{}{}_{secret}", kind.prefix(), id.simple()),
    })
}

/// `N` random bytes from the operating system.
pub(crate) fn random<const N: usize>() -> Result<[u8; N], Problem> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|_| Problem::internal())?;
    Ok(bytes)
}

/// Hex SHA-256 of a secret: what the database stores.
fn hash(secret: &str) -> String {
    hex(&Sha256::digest(secret.as_bytes()))
}

/// The ID in a secret of this kind, if it's well formed and its hash is
/// `stored`, compared in constant time.
pub fn check(kind: Kind, presented: &str, stored: impl FnOnce(Uuid) -> Option<String>) -> bool {
    let Some((id, secret)) = parse(kind, presented) else {
        return false;
    };
    stored(id).is_some_and(|h| bool::from(hash(secret).as_bytes().ct_eq(h.as_bytes())))
}

/// The ID in a well-formed secret of this kind.
pub fn id_of(kind: Kind, presented: &str) -> Option<Uuid> {
    parse(kind, presented).map(|(id, _)| id)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Splits `<prefix><id>_<secret>`; `None` if malformed.
fn parse(kind: Kind, presented: &str) -> Option<(Uuid, &str)> {
    let (id, secret) = presented.strip_prefix(kind.prefix())?.split_once('_')?;
    let valid_secret = secret.len() == 64 && secret.bytes().all(|b| b.is_ascii_hexdigit());
    (id.len() == 32 && valid_secret)
        .then(|| Uuid::try_parse(id).ok().map(|u| (u, secret)))
        .flatten()
}

/// Middleware for every `/api/v1` route.
pub async fn require(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, Problem> {
    // Inside the nested router the path is relative to `/api/v1`.
    let path = req.uri().path();
    if path == "/auth" || path.starts_with("/auth/") {
        return Ok(next.run(req).await);
    }
    let presented = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            let (scheme, token) = v.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
        })
        .ok_or_else(Problem::unauthorized)?;

    let principal = if let Some(boot) = &state.bootstrap_token
        && bool::from(presented.as_bytes().ct_eq(boot.as_bytes()))
    {
        let path = req.uri().path();
        let managing = ["/tokens", "/users"]
            .iter()
            .any(|p| path == *p || path.strip_prefix(p).is_some_and(|r| r.starts_with('/')));
        if !managing {
            return Err(Problem::forbidden(
                "the bootstrap token can only manage users and tokens; sign in or use a token",
            ));
        }
        Principal::Bootstrap
    } else {
        let id = id_of(Kind::Token, presented).ok_or_else(Problem::unauthorized)?;
        let row = store::tokens::find(&state.pool, TokenId::from_uuid(id))
            .await?
            .filter(|t| t.active(OffsetDateTime::now_utc()))
            .ok_or_else(Problem::unauthorized)?;
        if !check(Kind::Token, presented, |_| Some(row.secret_hash)) {
            return Err(Problem::unauthorized());
        }
        Principal::Token(row.id)
    };

    req.extensions_mut().insert(principal);
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_secrets_parse_back() {
        let t = generate(Kind::Token).unwrap();
        let (id, secret) = parse(Kind::Token, &t.text).unwrap();
        assert_eq!(id, t.id);
        assert_eq!(t.hash, hash(secret));
        assert_eq!(t.hash.len(), 64);
        assert!(!t.hash.contains(secret));
        assert!(check(Kind::Token, &t.text, |i| (i == t.id)
            .then(|| t.hash.clone())));
    }

    #[test]
    fn a_secret_is_only_good_for_its_kind() {
        let invite = generate(Kind::Invite).unwrap();
        assert!(invite.text.starts_with("exi_"));
        assert!(parse(Kind::Token, &invite.text).is_none());
        assert!(!check(Kind::Token, &invite.text, |_| Some(
            invite.hash.clone()
        )));
        assert!(!check(
            Kind::Invite,
            &generate(Kind::Invite).unwrap().text,
            |_| Some(invite.hash.clone())
        ));
    }

    #[test]
    fn malformed_tokens() {
        let good = generate(Kind::Token).unwrap().text;
        for bad in [
            "",
            "exq_",
            &good[4..],
            &good[..good.len() - 1],
            &good.replace("exq_", "xyz_"),
            &format!("{good}0"),
            "exq_0123456789abcdef0123456789abcdef_zz",
        ] {
            assert!(parse(Kind::Token, bad).is_none(), "{bad}");
        }
    }
}
