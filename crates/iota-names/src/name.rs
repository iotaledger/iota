// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, str::FromStr};

use iota_types::base_types::IotaAddress;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    constants::{
        IOTA_NAMES_MAX_LABEL_LENGTH, IOTA_NAMES_MAX_NAME_LENGTH, IOTA_NAMES_MIN_LABEL_LENGTH,
        IOTA_NAMES_SEPARATOR_AT, IOTA_NAMES_SEPARATOR_DOT, IOTA_NAMES_TLN,
    },
    error::IotaNamesError,
};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct Name {
    // Labels of the name, in reverse order
    labels: Vec<String>,
}

impl FromStr for Name {
    type Err = IotaNamesError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > IOTA_NAMES_MAX_NAME_LENGTH {
            return Err(IotaNamesError::NameLengthExceeded(
                s.len(),
                IOTA_NAMES_MAX_NAME_LENGTH,
            ));
        }

        let formatted_string = convert_from_at_format(s, &IOTA_NAMES_SEPARATOR_DOT)?;

        let labels = formatted_string
            .split(IOTA_NAMES_SEPARATOR_DOT)
            .rev()
            .map(validate_label)
            .collect::<Result<Vec<_>, Self::Err>>()?;

        // A valid name in our system has at least a TLN and an SLN (len == 2).
        if labels.len() < 2 {
            return Err(IotaNamesError::NotEnoughLabels);
        }

        if labels[0] != IOTA_NAMES_TLN {
            return Err(IotaNamesError::InvalidTln(labels[0].to_string()));
        }

        let labels = labels.into_iter().map(ToOwned::to_owned).collect();

        Ok(Name { labels })
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We use to_string() to check on-chain state and parse on-chain data
        // so we should always default to DOT format.
        let output = self.format(NameFormat::Dot);
        f.write_str(&output)?;

        Ok(())
    }
}

impl Name {
    pub fn type_(package_address: IotaAddress) -> StructTag {
        const IOTA_NAMES_NAME_MODULE: &IdentStr = ident_str!("name");
        const IOTA_NAMES_NAME_STRUCT: &IdentStr = ident_str!("Name");

        StructTag {
            address: package_address.into(),
            module: IOTA_NAMES_NAME_MODULE.to_owned(),
            name: IOTA_NAMES_NAME_STRUCT.to_owned(),
            type_params: vec![],
        }
    }

    /// Derive the parent name for a given name. Only subnames have
    /// parents; second-level names return `None`.
    ///
    /// ```
    /// # use std::str::FromStr;
    /// # use iota_names::name::Name;
    /// assert_eq!(
    ///     Name::from_str("test.example.iota").unwrap().parent(),
    ///     Some(Name::from_str("example.iota").unwrap())
    /// );
    /// assert_eq!(
    ///     Name::from_str("sub.test.example.iota").unwrap().parent(),
    ///     Some(Name::from_str("test.example.iota").unwrap())
    /// );
    /// assert_eq!(Name::from_str("example.iota").unwrap().parent(), None);
    /// ```
    pub fn parent(&self) -> Option<Self> {
        if self.is_subname() {
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

    /// Returns whether this name is a second-level name (Ex. `test.iota`)
    pub fn is_sln(&self) -> bool {
        self.num_labels() == 2
    }

    /// Returns whether this name is a subname (Ex. `sub.test.iota`)
    pub fn is_subname(&self) -> bool {
        self.num_labels() >= 3
    }

    /// Returns the number of labels including TLN.
    ///
    /// ```
    /// # use std::str::FromStr;
    /// # use iota_names::name::Name;
    /// assert_eq!(Name::from_str("test.example.iota").unwrap().num_labels(), 3)
    /// ```
    pub fn num_labels(&self) -> usize {
        self.labels.len()
    }

    /// Get the label at the given index
    pub fn label(&self, index: usize) -> Option<&String> {
        self.labels.get(index)
    }

    /// Get all of the labels. NOTE: These are in reverse order starting with
    /// the top-level name and proceeding to subnames.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Formats a name into a string based on the available output formats.
    /// The default separator is `.`
    pub fn format(&self, format: NameFormat) -> String {
        let mut labels = self.labels.clone();
        let sep = &IOTA_NAMES_SEPARATOR_DOT.to_string();
        labels.reverse();

        if format == NameFormat::Dot {
            // DOT format, all labels joined together with dots, including the TLN.
            labels.join(sep)
        } else {
            // SAFETY: This is a safe operation because we only allow a
            // name's label vector size to be >= 2 (see `Name::from_str`)
            let _tln = labels.pop();
            let sln = labels.pop().unwrap();

            // AT format, labels minus SLN joined together with dots, then joined to SLN
            // with @, no TLN.
            format!("{}{IOTA_NAMES_SEPARATOR_AT}{sln}", labels.join(sep))
        }
    }
}

/// Two different view options for a name.
/// `At` -> `test@example` | `Dot` -> `test.example.iota`
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum NameFormat {
    At,
    Dot,
}

/// Converts @label ending to label{separator}iota ending.
///
/// E.g. `@example` -> `example.iota` | `test@example` -> `test.example.iota`
fn convert_from_at_format(s: &str, separator: &char) -> Result<String, IotaNamesError> {
    let mut splits = s.split(IOTA_NAMES_SEPARATOR_AT);

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
    parts.push(IOTA_NAMES_TLN);

    Ok(parts.join(&separator.to_string()))
}

/// Checks the validity of a label according to these rules:
/// - length must be in
///   [IOTA_NAMES_MIN_LABEL_LENGTH..IOTA_NAMES_MAX_LABEL_LENGTH]
/// - must contain only '0'..'9', 'a'..'z' and '-'
/// - must not start or end with '-'
pub fn validate_label(label: &str) -> Result<&str, IotaNamesError> {
    let bytes = label.as_bytes();
    let len = bytes.len();

    if !(IOTA_NAMES_MIN_LABEL_LENGTH..=IOTA_NAMES_MAX_LABEL_LENGTH).contains(&len) {
        return Err(IotaNamesError::InvalidLabelLength(
            len,
            IOTA_NAMES_MIN_LABEL_LENGTH,
            IOTA_NAMES_MAX_LABEL_LENGTH,
        ));
    }

    for (i, character) in bytes.iter().enumerate() {
        match character {
            b'a'..=b'z' | b'0'..=b'9' => continue,
            b'-' => {
                if i == 0 || i == len - 1 {
                    return Err(IotaNamesError::HyphensAsFirstOrLastLabelChar);
                }
            }
            _ => return Err(IotaNamesError::InvalidLabelChar((*character) as char, i)),
        };
    }

    Ok(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_extraction() {
        let name = Name::from_str("leaf.node.test.iota")
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
        assert_eq!("@test".parse::<Name>().unwrap().to_string(), "test.iota");
        assert_eq!(
            "test.iota".parse::<Name>().unwrap().to_string(),
            "test.iota"
        );
        assert_eq!(
            "test@sln".parse::<Name>().unwrap().to_string(),
            "test.sln.iota"
        );
        assert_eq!(
            "test.test@example".parse::<Name>().unwrap().to_string(),
            "test.test.example.iota"
        );
        assert_eq!(
            "test.test-with-hyphen@example-hyphen"
                .parse::<Name>()
                .unwrap()
                .to_string(),
            "test.test-with-hyphen.example-hyphen.iota"
        );
        assert_eq!(
            "iota@iota".parse::<Name>().unwrap().to_string(),
            "iota.iota.iota"
        );
        assert_eq!("@iota".parse::<Name>().unwrap().to_string(), "iota.iota");
        assert_eq!(
            "test.test.iota".parse::<Name>().unwrap().to_string(),
            "test.test.iota"
        );
        assert_eq!(
            "test.test.test.iota".parse::<Name>().unwrap().to_string(),
            "test.test.test.iota"
        );
        assert_eq!(
            "test.test-with-hyphen.test-with-hyphen.iota"
                .parse::<Name>()
                .unwrap()
                .to_string(),
            "test.test-with-hyphen.test-with-hyphen.iota"
        );
    }

    #[test]
    fn invalid_inputs() {
        assert!(".".parse::<Name>().is_err());
        assert!("@".parse::<Name>().is_err());
        assert!("@inner.iota".parse::<Name>().is_err());
        assert!("test@".parse::<Name>().is_err());
        assert!("iota".parse::<Name>().is_err());
        assert!("test.test@example.iota".parse::<Name>().is_err());
        assert!("test@test@example".parse::<Name>().is_err());
        assert!("test.atoi".parse::<Name>().is_err());
        assert!("test.test@example-".parse::<Name>().is_err());
        assert!("test.test@-example".parse::<Name>().is_err());
        assert!("test.test-@example".parse::<Name>().is_err());
        assert!("test.-test@example".parse::<Name>().is_err());
        assert!("test.test-.iota".parse::<Name>().is_err());
        assert!("test.-test.iota".parse::<Name>().is_err());
    }

    #[test]
    fn outputs() {
        let mut name = "test.iota".parse::<Name>().unwrap();
        assert!(name.format(NameFormat::Dot) == "test.iota");
        assert!(name.format(NameFormat::At) == "@test");

        name = "test.test.iota".parse::<Name>().unwrap();
        assert!(name.format(NameFormat::Dot) == "test.test.iota");
        assert!(name.format(NameFormat::At) == "test@test");

        name = "test.test.test.iota".parse::<Name>().unwrap();
        assert!(name.format(NameFormat::Dot) == "test.test.test.iota");
        assert!(name.format(NameFormat::At) == "test.test@test");

        name = "test.test.test.test.iota".parse::<Name>().unwrap();
        assert!(name.format(NameFormat::Dot) == "test.test.test.test.iota");
        assert!(name.format(NameFormat::At) == "test.test.test@test");
    }
}
