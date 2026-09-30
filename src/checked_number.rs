//! Numbers that only hold values wreq can use, like an odd `initial_stream_id`.

/// Declares a number type that rejects some values, with the error type returned for them.
///
/// wreq and http2 panic on these values when a connection starts, so the type can't be created
/// with one. On the wire it is a plain number: serialization writes the number, and
/// deserialization reads one and rejects it with the error's message if the check fails.
macro_rules! checked_number {
    (
        $(#[$meta:meta])*
        pub struct $name:ident($int:ty);

        $(#[$error_meta:meta])*
        pub struct $error:ident: $($message:tt)+
    ) => {
        $(#[$error_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
        #[error($($message)+)]
        pub struct $error(pub $int);

        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($int);

        impl $name {
            /// Creates the value if the check passes.
            ///
            /// # Errors
            ///
            #[doc = concat!("Returns [`", stringify!($error), "`] if it does not.")]
            pub const fn new(value: $int) -> Result<Self, $error> {
                if Self::is_valid(value) {
                    Ok(Self(value))
                } else {
                    Err($error(value))
                }
            }

            /// The value as a plain number.
            #[must_use]
            pub const fn get(self) -> $int {
                self.0
            }
        }

        impl TryFrom<$int> for $name {
            type Error = $error;

            fn try_from(value: $int) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for $int {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serde::Serialize::serialize(&self.0, serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <$int as serde::Deserialize>::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(crate) use checked_number;
