// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, str::FromStr};

use iota_types::base_types::IotaAddress;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    config::{ACCEPTED_SEPARATORS, DEFAULT_TLD, IOTA_NEW_FORMAT_SEPARATOR},
    error::IotaNamesError,
};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct Domain {
    // Labels of the domain name, in reverse order
    labels: Vec<String>,
}

// impl std::fmt::Display for Domain {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let labels = self.labels.iter().rev().cloned().collect::<Vec<_>>();
//         write!(f, "{}", labels.join("."))
//     }
// }

// impl FromStr for Domain {
//     type Err = anyhow::Error;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         const VALID_TLDS: &[&str] = &["iota"];
//         let mut segments = s.split('.').collect::<Vec<_>>();
//         anyhow::ensure!(segments.len() >= 2, "domain has too few labels");
//         let tld = segments.pop().unwrap();
//         anyhow::ensure!(VALID_TLDS.contains(&tld), "invalid TLD: {tld}");
//         let mut labels = vec![tld.to_owned()];
//         for segment in segments.into_iter().rev() {
//             labels.push(parse_domain_label(segment)?);
//         }
//         Ok(Self { labels })
//     }
// }

impl FromStr for Domain {
    type Err = IotaNamesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        /// The maximum length of a full domain
        const MAX_DOMAIN_LENGTH: usize = 235;

        if s.len() > MAX_DOMAIN_LENGTH {
            return Err(IotaNamesError::ExceedsMaxLength(s.len(), MAX_DOMAIN_LENGTH));
        }
        let separator = separator(s)?;

        let formatted_string = convert_from_new_format(s, &separator)?;

        let labels = formatted_string
            .split(separator)
            .rev()
            .map(validate_label)
            .collect::<Result<Vec<_>, Self::Err>>()?;

        // A valid domain in our system has at least a TLD and an SLD (len == 2).
        if labels.len() < 2 {
            return Err(IotaNamesError::LabelsEmpty);
        }

        let labels = labels.into_iter().map(ToOwned::to_owned).collect();
        Ok(Domain { labels })
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We use to_string() to check on-chain state and parse on-chain data
        // so we should always default to DOT format.
        let output = self.format(DomainFormat::Dot);
        f.write_str(&output)?;

        Ok(())
    }
}

impl Domain {
    pub fn type_(package_address: IotaAddress) -> StructTag {
        const IOTA_NAMES_DOMAIN_MODULE: &IdentStr = ident_str!("domain");
        const IOTA_NAMES_DOMAIN_STRUCT: &IdentStr = ident_str!("Domain");

        StructTag {
            address: package_address.into(),
            module: IOTA_NAMES_DOMAIN_MODULE.to_owned(),
            name: IOTA_NAMES_DOMAIN_STRUCT.to_owned(),
            type_params: vec![],
        }
    }

    /// Derive the parent domain for a given domain. Only subdomains have
    /// parents; second-level domains return `None`.
    ///
    /// ```
    /// # use std::str::FromStr;
    /// # use iota_names::domain::Domain;
    /// assert_eq!(
    ///     Domain::from_str("test.example.iota").unwrap().parent(),
    ///     Some(Domain::from_str("example.iota").unwrap())
    /// );
    /// assert_eq!(
    ///     Domain::from_str("sub.test.example.iota").unwrap().parent(),
    ///     Some(Domain::from_str("test.example.iota").unwrap())
    /// );
    /// assert_eq!(Domain::from_str("example.iota").unwrap().parent(), None);
    /// ```
    pub fn parent(&self) -> Option<Self> {
        if self.is_subdomain() {
            Some(Self {
                labels: self
                    .labels
                    .iter()
                    .take(self.num_labels() - 1)
                    .cloned()
                    .collect(),
            })
        } else {
            None
        }
    }

    /// Returns whether this domain is a second-level domain (Ex. `test.iota`)
    pub fn is_sld(&self) -> bool {
        self.num_labels() == 2
    }

    /// Returns whether this domain is a subdomain (Ex. `sub.test.iota`)
    pub fn is_subdomain(&self) -> bool {
        self.num_labels() >= 3
    }

    /// Returns the number of labels including TLD.
    ///
    /// ```
    /// assert_eq!(
    ///     Domain::from_str("test.example.iota").unwrap().num_labels(),
    ///     3
    /// )
    /// ```
    pub fn num_labels(&self) -> usize {
        self.labels.len()
    }

    /// Get the label at the given index
    pub fn label(&self, index: usize) -> Option<&String> {
        self.labels.get(index)
    }

    /// Get all of the labels. NOTE: These are in reverse order starting with
    /// the top-level domain and proceeding to subdomains.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Formats a domain into a string based on the available output formats.
    /// The default separator is `.`
    pub fn format(&self, format: DomainFormat) -> String {
        let mut labels = self.labels.clone();
        let sep = &ACCEPTED_SEPARATORS[0].to_string();
        labels.reverse();

        if format == DomainFormat::Dot {
            return labels.join(sep);
        };

        // SAFETY: This is a safe operation because we only allow a
        // domain's label vector size to be >= 2 (see `Domain::from_str`)
        let _tld = labels.pop();
        let sld = labels.pop().unwrap();

        format!("{}{}{}", labels.join(sep), IOTA_NEW_FORMAT_SEPARATOR, sld)
    }
}

/// Two different view options for a domain.
/// `At` -> `test@example` | `Dot` -> `test.example.iota`
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum DomainFormat {
    At,
    Dot,
}

/// Parses a separator from the domain string input.
/// E.g.  `example.iota` -> `.` | example*iota -> `@` | `example*iota` -> `*`
fn separator(s: &str) -> Result<char, IotaNamesError> {
    let mut domain_separator: Option<char> = None;

    for separator in ACCEPTED_SEPARATORS.iter() {
        if s.contains(*separator) {
            if domain_separator.is_some() {
                return Err(IotaNamesError::InvalidSeparator);
            }

            domain_separator = Some(*separator);
        }
    }

    match domain_separator {
        Some(separator) => Ok(separator),
        None => Ok(ACCEPTED_SEPARATORS[0]),
    }
}

/// Converts @label ending to label{separator}iota ending.
///
/// E.g. `@example` -> `example.iota` | `test@example` -> `test.example.iota`
fn convert_from_new_format(s: &str, separator: &char) -> Result<String, IotaNamesError> {
    let mut splits = s.split(IOTA_NEW_FORMAT_SEPARATOR);

    let Some(before) = splits.next() else {
        return Err(IotaNamesError::InvalidSeparator);
    };

    let Some(after) = splits.next() else {
        return Ok(before.to_string());
    };

    if splits.next().is_some() || after.contains(*separator) || after.is_empty() {
        return Err(IotaNamesError::InvalidSeparator);
    }

    let mut parts = vec![];

    if !before.is_empty() {
        parts.push(before);
    }

    parts.push(after);
    parts.push(DEFAULT_TLD);

    Ok(parts.join(&separator.to_string()))
}

pub fn validate_label(label: &str) -> Result<&str, IotaNamesError> {
    const MIN_LABEL_LENGTH: usize = 1;
    const MAX_LABEL_LENGTH: usize = 63;
    let bytes = label.as_bytes();
    let len = bytes.len();

    if !(MIN_LABEL_LENGTH..=MAX_LABEL_LENGTH).contains(&len) {
        return Err(IotaNamesError::InvalidLength(
            len,
            MIN_LABEL_LENGTH,
            MAX_LABEL_LENGTH,
        ));
    }

    for (i, character) in bytes.iter().enumerate() {
        let is_valid_character = match character {
            b'a'..=b'z' => true,
            b'0'..=b'9' => true,
            b'-' if i != 0 && i != len - 1 => true,
            _ => false,
        };

        if !is_valid_character {
            match character {
                b'-' => return Err(IotaNamesError::InvalidHyphens),
                _ => return Err(IotaNamesError::InvalidUnderscore),
            }
        };
    }
    Ok(label)
}

// fn parse_domain_label(label: &str) -> anyhow::Result<String> {
//     anyhow::ensure!(
//         label.len() >= MIN_LABEL_LEN && label.len() <= MAX_LABEL_LEN,
//         "label length outside allowed range
// [{MIN_LABEL_LEN}..{MAX_LABEL_LEN}]: {}",         label.len()
//     );
//     let regex =
// regex::Regex::new("^[a-zA-Z0-9][a-zA-Z0-9-]+[a-zA-Z0-9]$").unwrap();

//     anyhow::ensure!(
//         regex.is_match(label),
//         "invalid characters in domain: {label}"
//     );
//     Ok(label.to_owned())
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_extraction() {
        let name = Domain::from_str("leaf.node.test.iota")
            .unwrap()
            .parent()
            .unwrap();

        assert_eq!(name.to_string(), "node.test.iota");

        let name = name.parent().unwrap();

        assert_eq!(name.to_string(), "test.iota");

        assert!(name.parent().is_none());
    }

    #[test]
    fn name_service_outputs() {
        assert_eq!("@test".parse::<Domain>().unwrap().to_string(), "test.iota");
        assert_eq!(
            "test.iota".parse::<Domain>().unwrap().to_string(),
            "test.iota"
        );
        assert_eq!(
            "test@sld".parse::<Domain>().unwrap().to_string(),
            "test.sld.iota"
        );
        assert_eq!(
            "test.test@example".parse::<Domain>().unwrap().to_string(),
            "test.test.example.iota"
        );
        assert_eq!(
            "iota@iota".parse::<Domain>().unwrap().to_string(),
            "iota.iota.iota"
        );

        assert_eq!("@iota".parse::<Domain>().unwrap().to_string(), "iota.iota");

        assert_eq!(
            "test*test@test".parse::<Domain>().unwrap().to_string(),
            "test.test.test.iota"
        );
        assert_eq!(
            "test.test.iota".parse::<Domain>().unwrap().to_string(),
            "test.test.iota"
        );
        assert_eq!(
            "test.test.test.iota".parse::<Domain>().unwrap().to_string(),
            "test.test.test.iota"
        );
    }

    #[test]
    fn different_wildcard() {
        assert_eq!("test.iota".parse::<Domain>(), "test*iota".parse::<Domain>(),);

        assert_eq!("@test".parse::<Domain>(), "test*iota".parse::<Domain>(),);
    }

    #[test]
    fn invalid_inputs() {
        assert!("*".parse::<Domain>().is_err());
        assert!(".".parse::<Domain>().is_err());
        assert!("@".parse::<Domain>().is_err());
        assert!("@inner.iota".parse::<Domain>().is_err());
        assert!("@inner*iota".parse::<Domain>().is_err());
        assert!("test@".parse::<Domain>().is_err());
        assert!("iota".parse::<Domain>().is_err());
        assert!("test.test@example.iota".parse::<Domain>().is_err());
        assert!("test@test@example".parse::<Domain>().is_err());
    }

    #[test]
    fn outputs() {
        let mut domain = "test.iota".parse::<Domain>().unwrap();
        assert!(domain.format(DomainFormat::Dot) == "test.iota");
        assert!(domain.format(DomainFormat::At) == "@test");

        domain = "test.test.iota".parse::<Domain>().unwrap();
        assert!(domain.format(DomainFormat::Dot) == "test.test.iota");
        assert!(domain.format(DomainFormat::At) == "test@test");

        domain = "test.test.test.iota".parse::<Domain>().unwrap();
        assert!(domain.format(DomainFormat::Dot) == "test.test.test.iota");
        assert!(domain.format(DomainFormat::At) == "test.test@test");

        domain = "test.test.test.test.iota".parse::<Domain>().unwrap();
        assert!(domain.format(DomainFormat::Dot) == "test.test.test.test.iota");
        assert!(domain.format(DomainFormat::At) == "test.test.test@test");
    }
}
