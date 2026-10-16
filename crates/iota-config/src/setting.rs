// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::Error as _};

/// A config field that may be absent, explicitly `null`, or set to a value.
///
/// `Option<T>` cannot tell "the operator said nothing" from "the operator
/// turned this off", and the two need different answers: the first takes a
/// default, the second does not. Use [`Setting::resolve`] to apply the default
/// and get back the `Option<T>` the rest of the code wants.
///
/// A field of this type is declared as:
///
/// ```ignore
/// #[serde(default, skip_serializing_if = "Setting::is_unset")]
/// pub max_connections: Setting<NonZeroUsize>,
/// ```
///
/// `skip_serializing_if` is not optional: serialising [`Setting::Unset`] is an
/// error, because writing it as `null` would turn a default into "off" the next
/// time the file was read.
///
/// Note that a field written with no value at all — `max-connections:` — is
/// YAML `null`, so it turns the setting **off** rather than leaving the
/// default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting<T> {
    /// The field was set to this value.
    Value(T),
    /// The field was absent, so the default applies.
    Unset,
    /// The field was `null`, so whatever it configures is off.
    Disabled,
}

impl<T> Setting<T> {
    /// The configured value, falling back to `default` where the field was
    /// absent. `None` means off.
    pub fn resolve(self, default: Option<T>) -> Option<T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Unset => default,
            Self::Disabled => None,
        }
    }

    /// Whether the field was absent. Meant for `skip_serializing_if`.
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }
}

impl<T> Default for Setting<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<T: Serialize> Serialize for Setting<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => serializer.serialize_some(value),
            Self::Disabled => serializer.serialize_none(),
            // Unreachable through a field that carries
            // `skip_serializing_if = "Setting::is_unset"`, and an error rather
            // than a guess because `null` is how "off" is spelled: writing one
            // for the other would change what the file means.
            Self::Unset => Err(S::Error::custom(
                "an unset field must carry `skip_serializing_if = \"Setting::is_unset\"`",
            )),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Setting<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // An absent field never reaches here: `#[serde(default)]` answers that
        // case with `Unset`. What arrives is either a value or `null`.
        match Option::<T>::deserialize(deserializer) {
            Ok(Some(value)) => Ok(Self::Value(value)),
            Ok(None) => Ok(Self::Disabled),
            Err(err) => Err(D::Error::custom(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, time::Duration};

    use super::*;

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    #[serde(rename_all = "kebab-case")]
    struct Limits {
        #[serde(default, skip_serializing_if = "Setting::is_unset")]
        max_connections: Setting<NonZeroUsize>,
        #[serde(default, skip_serializing_if = "Setting::is_unset")]
        max_connection_idle: Setting<Duration>,
    }

    fn parse(yaml: &str) -> Result<Limits, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    #[test]
    fn an_absent_field_is_unset() {
        let limits = parse("{}").unwrap();
        assert_eq!(limits.max_connections, Setting::Unset);
        assert_eq!(limits.max_connection_idle, Setting::Unset);
    }

    #[test]
    fn a_value_is_the_value() {
        let limits = parse("max-connections: 8192").unwrap();
        assert_eq!(
            limits.max_connections,
            Setting::Value(NonZeroUsize::new(8192).unwrap())
        );
    }

    #[test]
    fn an_explicit_null_is_disabled() {
        let limits = parse("max-connections: null").unwrap();
        assert_eq!(limits.max_connections, Setting::Disabled);
    }

    /// A field written with no value is YAML `null`, so it reads as off rather
    /// than as the default. Pinned because it is the one state an operator may
    /// not expect.
    #[test]
    fn a_field_with_no_value_is_disabled() {
        let limits = parse("max-connections:").unwrap();
        assert_eq!(limits.max_connections, Setting::Disabled);
    }

    /// Zero is not a way to turn a count off; `null` is. `NonZeroUsize` is what
    /// rejects it, and it is rejected where it is written rather than later.
    #[test]
    fn a_zero_count_is_refused() {
        let error = parse("max-connections: 0").unwrap_err().to_string();
        assert!(
            error.contains("max-connections"),
            "the error should name the field, got: {error}"
        );
    }

    #[test]
    fn resolve_applies_the_default_only_when_unset() {
        let default = NonZeroUsize::new(64);
        assert_eq!(
            Setting::Value(NonZeroUsize::new(8).unwrap()).resolve(default),
            NonZeroUsize::new(8)
        );
        assert_eq!(Setting::Unset.resolve(default), default);
        assert_eq!(Setting::<NonZeroUsize>::Disabled.resolve(default), None);
    }

    #[test]
    fn every_state_survives_a_round_trip() {
        for limits in [
            Limits {
                max_connections: Setting::Unset,
                max_connection_idle: Setting::Unset,
            },
            Limits {
                max_connections: Setting::Value(NonZeroUsize::new(8192).unwrap()),
                max_connection_idle: Setting::Value(Duration::from_secs(300)),
            },
            Limits {
                max_connections: Setting::Disabled,
                max_connection_idle: Setting::Disabled,
            },
            Limits {
                max_connections: Setting::Value(NonZeroUsize::new(1).unwrap()),
                max_connection_idle: Setting::Disabled,
            },
        ] {
            let yaml = serde_yaml::to_string(&limits).unwrap();
            assert_eq!(parse(&yaml).unwrap(), limits, "round trip of {yaml}");
        }
    }

    /// A fully defaulted struct writes nothing, which is what keeps a config
    /// file free of settings nobody chose.
    #[test]
    fn an_unset_field_is_not_written() {
        let yaml = serde_yaml::to_string(&Limits {
            max_connections: Setting::Unset,
            max_connection_idle: Setting::Value(Duration::from_secs(1)),
        })
        .unwrap();
        assert!(
            !yaml.contains("max-connections"),
            "an unset field should be skipped, got: {yaml}"
        );
    }

    /// The guard above is a real requirement, not a style note.
    #[test]
    fn serialising_an_unset_field_without_the_guard_is_an_error() {
        #[derive(Serialize)]
        struct Unguarded {
            max_connections: Setting<NonZeroUsize>,
        }

        let error = serde_yaml::to_string(&Unguarded {
            max_connections: Setting::Unset,
        })
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("skip_serializing_if"),
            "the error should say what is missing, got: {error}"
        );
    }
}
