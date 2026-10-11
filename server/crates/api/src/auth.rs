//! Bearer-token authentication (design §10 tier 1).
//!
//! A token looks like `exq_<id>_<secret>`: 32 hex digits of token ID, then 64
//! hex digits (256 bits) of random secret. The server stores only an argon2
//! hash of the secret, finds the token by its ID, and verifies the secret.
//!
//! The bootstrap token (a podman secret) is accepted only on `/tokens`, to
//! create the first real tokens.

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use domain::TokenId;
use subtle::ConstantTimeEq;
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

/// A freshly generated token. `token` is shown to the user exactly once.
pub struct NewToken {
    pub id: TokenId,
    pub token: String,
    pub secret_hash: String,
}

/// Generates a token and hashes its secret. CPU-heavy: call off the async
/// runtime (`spawn_blocking`).
pub fn generate() -> Result<NewToken, Problem> {
    let id = TokenId::new();
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Problem::internal())?;
    let secret = hex(&bytes);
    let secret_hash = Argon2::default()
        .hash_password(secret.as_bytes())
        .map_err(|_| Problem::internal())?
        .to_string();
    Ok(NewToken {
        id,
        token: format!("exq_{}_{secret}", id.as_uuid().simple()),
        secret_hash,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Splits `exq_<id>_<secret>`; `None` if malformed.
fn parse(token: &str) -> Option<(TokenId, &str)> {
    let (id, secret) = token.strip_prefix("exq_")?.split_once('_')?;
    let valid_secret = secret.len() == 64 && secret.bytes().all(|b| b.is_ascii_hexdigit());
    (id.len() == 32 && valid_secret)
        .then(|| {
            Uuid::try_parse(id)
                .ok()
                .map(|u| (TokenId::from_uuid(u), secret))
        })
        .flatten()
}

/// Middleware for every `/api/v1` route.
pub async fn require(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, Problem> {
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
        // Inside the nested router the path is relative to `/api/v1`.
        let path = req.uri().path();
        if path != "/tokens" && !path.starts_with("/tokens/") {
            return Err(Problem::forbidden(
                "the bootstrap token can only manage tokens; create a token and use it instead",
            ));
        }
        Principal::Bootstrap
    } else {
        let (id, secret) = parse(presented).ok_or_else(Problem::unauthorized)?;
        let row = store::tokens::find(&state.pool, id)
            .await?
            .filter(|t| t.revoked_at.is_none())
            .ok_or_else(Problem::unauthorized)?;
        let secret = secret.to_owned();
        let valid = tokio::task::spawn_blocking(move || {
            PasswordHash::new(&row.secret_hash).is_ok_and(|h| {
                Argon2::default()
                    .verify_password(secret.as_bytes(), &h)
                    .is_ok()
            })
        })
        .await
        .map_err(|_| Problem::internal())?;
        if !valid {
            return Err(Problem::unauthorized());
        }
        Principal::Token(id)
    };

    req.extensions_mut().insert(principal);
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_parse_back() {
        let t = generate().unwrap();
        let (id, secret) = parse(&t.token).unwrap();
        assert_eq!(id, t.id);
        assert!(t.secret_hash.starts_with("$argon2id$"));
        assert!(!t.secret_hash.contains(secret));
    }

    #[test]
    fn malformed_tokens() {
        let good = generate().unwrap().token;
        for bad in [
            "",
            "exq_",
            &good[4..],
            &good[..good.len() - 1],
            &good.replace("exq_", "xyz_"),
            &format!("{good}0"),
            "exq_0123456789abcdef0123456789abcdef_zz",
        ] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }
}
