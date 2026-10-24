//! Typed identifiers. Each entity gets its own ID type, so passing an
//! `AccountId` where a `PartyId` is expected fails to compile.
//!
//! All of them wrap a UUIDv7: IDs can be created before anything reaches the
//! database (import staging builds whole transactions first), and v7 values
//! sort by creation time, which keeps database indexes compact. The inner type
//! is defined only in the macro below.

use std::fmt;

use uuid::Uuid;

macro_rules! id_type {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Uuid);

        impl $name {
            /// A fresh ID, ordered after every ID created before it.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an ID that already exists (e.g. read from the database).
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    )*};
}

id_type!(
    CommodityId,
    PartyId,
    AccountId,
    CounterpartyId,
    TransactionId,
    CategoryId,
    BillId,
    TokenId,
    UserId,
    PasskeyId,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_sort_by_creation() {
        let a = AccountId::new();
        let b = AccountId::new();
        assert!(a < b);
    }
}
