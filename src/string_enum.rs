//! Enums that travel as fixed strings, like `"TLS_1_3"` or `"SERVER_NAME"`.

/// A string that is not one of the names an enum accepts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind}: `{value}`")]
pub struct UnknownVariant {
    kind: &'static str,
    value: String,
}

impl UnknownVariant {
    pub(crate) fn new(kind: &'static str, value: &str) -> Self {
        Self {
            kind,
            value: value.to_owned(),
        }
    }
}

/// Declares an enum whose serde form is a fixed string per variant.
///
/// Serialization writes a plain string and deserialization reads a `String`, the same way the
/// fork's hand-written impls do, so the format matches for any serde backend, not just JSON.
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident ($kind:literal) {
            $($(#[$variant_meta:meta])* $variant:ident => $wire:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(into = "&'static str", try_from = "String")]
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// The name used on the wire.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }

        impl From<$name> for &'static str {
            fn from(value: $name) -> Self {
                value.as_str()
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::string_enum::UnknownVariant;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err($crate::string_enum::UnknownVariant::new($kind, value)),
                }
            }
        }

        impl TryFrom<String> for $name {
            type Error = $crate::string_enum::UnknownVariant;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                value.parse()
            }
        }
    };
}

pub(crate) use string_enum;
