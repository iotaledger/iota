// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    fmt::{Display, Formatter, Write},
    fs,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow, bail, ensure};
use bip32::DerivationPath;
use bip39::{Language, Mnemonic, Seed};
use iota_sdk_types::crypto::{Intent, IntentMessage};
use iota_types::{
    base_types::IotaAddress,
    crypto::{
        EncodeDecodeBase64, IotaKeyPair, PublicKey, Signature, SignatureScheme, enum_dispatch,
        get_key_pair_from_rng,
    },
};
use rand::{SeedableRng, rngs::StdRng};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DisplayFromStr, serde_as};
use tracing::{debug, info};

use crate::{
    key_derive::{derive_key_pair_from_path, generate_new_key},
    random_names::{random_name, random_names},
    serde_iota_keypair, serde_public_key,
};

#[derive(Serialize, Deserialize)]
#[enum_dispatch(AccountKeystore)]
pub enum Keystore {
    File(FileBasedKeystore),
    InMem(InMemKeystore),
}

#[enum_dispatch]
pub trait AccountKeystore: Send + Sync {
    fn add_key(
        &mut self,
        alias: Option<String>,
        key: impl Into<StoredKey>,
    ) -> Result<(), anyhow::Error>;
    fn remove_key(&mut self, address: &IotaAddress) -> Result<(), anyhow::Error>;
    fn keys(&self) -> Vec<&StoredKey>;
    fn get_key(&self, address: &IotaAddress) -> Result<&StoredKey, anyhow::Error>;

    fn sign_hashed(&self, address: &IotaAddress, msg: &[u8])
    -> Result<Signature, signature::Error>;

    fn sign_secure<T>(
        &self,
        address: &IotaAddress,
        msg: &T,
        intent: Intent,
    ) -> Result<Signature, signature::Error>
    where
        T: Serialize;
    fn addresses(&self) -> Vec<IotaAddress> {
        self.keys().into_iter().map(|k| k.address()).collect()
    }
    fn addresses_with_alias(&self) -> Vec<(&IotaAddress, &Alias)>;
    fn aliases(&self) -> Vec<&Alias>;
    fn aliases_mut(&mut self) -> Vec<&mut Alias>;
    fn alias_names(&self) -> Vec<&str> {
        self.aliases()
            .into_iter()
            .map(|a| a.alias.as_str())
            .collect()
    }
    /// Get alias of address
    fn get_alias_by_address(&self, address: &IotaAddress) -> Result<String, anyhow::Error>;
    fn get_address_by_alias(&self, alias: String) -> Result<&IotaAddress, anyhow::Error>;
    /// Check if an alias exists by its name
    fn alias_exists(&self, alias: &str) -> bool {
        self.alias_names().contains(&alias)
    }

    fn create_alias(&self, alias: Option<String>) -> Result<String, anyhow::Error>;

    fn update_alias(
        &mut self,
        old_alias: &str,
        new_alias: Option<&str>,
    ) -> Result<String, anyhow::Error>;

    // Internal function. Use update_alias instead
    fn update_alias_value(
        &mut self,
        old_alias: &str,
        new_alias: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        if !self.alias_exists(old_alias) {
            bail!("The provided alias {old_alias} does not exist");
        }

        let new_alias_name = self.create_alias(new_alias.map(str::to_string))?;

        for a in self.aliases_mut() {
            if a.alias == old_alias {
                *a = Alias {
                    alias: new_alias_name.clone(),
                };
            }
        }
        Ok(new_alias_name)
    }

    fn generate_and_add_new_key(
        &mut self,
        key_scheme: SignatureScheme,
        alias: Option<String>,
        derivation_path: Option<DerivationPath>,
        word_length: Option<String>,
    ) -> Result<(IotaAddress, String, SignatureScheme), anyhow::Error> {
        let (address, kp, scheme, phrase) =
            generate_new_key(key_scheme, derivation_path, word_length)?;
        self.add_key(alias, kp)?;
        Ok((address, phrase, scheme))
    }

    fn import_from_mnemonic(
        &mut self,
        phrase: &str,
        key_scheme: SignatureScheme,
        derivation_path: Option<DerivationPath>,
        alias: Option<String>,
    ) -> Result<IotaAddress, anyhow::Error> {
        let mnemonic = Mnemonic::from_phrase(phrase, Language::English)
            .map_err(|e| anyhow::anyhow!("Invalid mnemonic phrase: {:?}", e))?;
        let seed = Seed::new(&mnemonic, "");
        self.import_from_seed(seed.as_bytes(), key_scheme, derivation_path, alias)
    }

    fn import_from_seed(
        &mut self,
        seed: &[u8],
        key_scheme: SignatureScheme,
        derivation_path: Option<DerivationPath>,
        alias: Option<String>,
    ) -> Result<IotaAddress, anyhow::Error> {
        match derive_key_pair_from_path(seed, derivation_path, &key_scheme) {
            Ok((address, kp)) => {
                self.add_key(alias, kp)?;
                Ok(address)
            }
            Err(e) => Err(anyhow!("error getting keypair {:?}", e)),
        }
    }

    fn import_from_external(
        &mut self,
        source: &str,
        public_key: PublicKey,
        derivation_path: Option<DerivationPath>,
        alias: Option<String>,
    ) -> Result<(), anyhow::Error> {
        self.add_key(
            alias,
            StoredKey::External {
                derivation_path,
                public_key,
                source: source.to_string(),
            },
        )
    }
}

impl Display for Keystore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();
        match self {
            Keystore::File(file) => {
                writeln!(writer, "Keystore Type: File")?;
                write!(writer, "Keystore Path : {:?}", file.path)?;
                write!(f, "{writer}")
            }
            Keystore::InMem(_) => {
                writeln!(writer, "Keystore Type: InMem")?;
                write!(f, "{writer}")
            }
        }
    }
}

// Used to migrate from keystore v1 to v2
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LegacyAlias {
    pub alias: String,
    pub public_key_base64: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Alias {
    pub alias: String,
}

#[expect(clippy::large_enum_variant)]
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(
    tag = "type",            // this makes {"type": "...", "value": …}
    content = "value",       // name the payload field "value"
    rename_all = "snake_case"
)]
pub enum StoredKey {
    #[serde(with = "serde_iota_keypair")]
    KeyPair(IotaKeyPair),
    Account(IotaAddress),
    External {
        source: String,
        #[serde_as(as = "Option<DisplayFromStr>")]
        #[serde(skip_serializing_if = "Option::is_none")]
        derivation_path: Option<DerivationPath>,
        #[serde(rename = "public_key_base64_with_flag", with = "serde_public_key")]
        public_key: PublicKey,
    },
}

impl From<IotaKeyPair> for StoredKey {
    fn from(keypair: IotaKeyPair) -> Self {
        StoredKey::KeyPair(keypair)
    }
}

impl StoredKey {
    pub fn address(&self) -> IotaAddress {
        match self {
            StoredKey::KeyPair(key) => (&key.public()).into(),
            StoredKey::Account(address) => *address,
            StoredKey::External { public_key, .. } => public_key.into(),
        }
    }

    pub fn public(&self) -> PublicKey {
        match self {
            StoredKey::KeyPair(keypair) => keypair.public(),
            StoredKey::Account(_) => panic!("Account addresses are not backed by key pairs."),
            StoredKey::External { public_key, .. } => public_key.clone(),
        }
    }

    pub fn derivation_path(&self) -> Option<DerivationPath> {
        match self {
            StoredKey::KeyPair(_) => None,
            StoredKey::Account(_) => None,
            StoredKey::External {
                derivation_path, ..
            } => derivation_path.clone(),
        }
    }

    pub fn external_source(&self) -> Option<String> {
        match self {
            StoredKey::KeyPair(_) => None,
            StoredKey::Account(_) => None,
            StoredKey::External { source, .. } => Some(source.clone()),
        }
    }

    pub fn as_keypair(&self) -> Result<&IotaKeyPair, anyhow::Error> {
        match self {
            StoredKey::KeyPair(keypair) => Ok(keypair),
            StoredKey::Account(_) => bail!("Account addresses are not backed by key pairs."),
            StoredKey::External { .. } => bail!("Cannot get key pair for External keys."),
        }
    }

    pub fn source(&self) -> &str {
        match self {
            StoredKey::KeyPair(_) => "keypair",
            StoredKey::Account(_) => "account",
            StoredKey::External { source, .. } => source,
        }
    }
}

#[derive(Default)]
pub struct FileBasedKeystore {
    keys: BTreeMap<IotaAddress, StoredKey>,
    aliases: BTreeMap<IotaAddress, Alias>,
    path: PathBuf,
}

impl Serialize for FileBasedKeystore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.path.to_str().unwrap_or(""))
    }
}

impl<'de> Deserialize<'de> for FileBasedKeystore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        FileBasedKeystore::new(&PathBuf::from(String::deserialize(deserializer)?))
            .map_err(D::Error::custom)
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct FileBasedKeystoreFile {
    pub version: u8,
    pub keys: Vec<AliasedKey>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AliasedKey {
    pub alias: String,
    pub address: IotaAddress,
    pub key: StoredKey,
}

impl AccountKeystore for FileBasedKeystore {
    fn sign_hashed(
        &self,
        address: &IotaAddress,
        msg: &[u8],
    ) -> Result<Signature, signature::Error> {
        let stored_key = self.keys.get(address).ok_or_else(|| {
            signature::Error::from_source(format!("Cannot find key for address: [{address}]"))
        })?;

        match stored_key {
            StoredKey::KeyPair(keypair) => Ok(Signature::new_hashed(msg, keypair)),
            StoredKey::Account(_) => Err(signature::Error::from_source(
                "sign_hashed is not supported for account type",
            )),
            StoredKey::External { source, .. } => Err(signature::Error::from_source(format!(
                "sign_hashed is not supported for external type: {source} [{address}]"
            ))),
        }
    }
    fn sign_secure<T>(
        &self,
        address: &IotaAddress,
        msg: &T,
        intent: Intent,
    ) -> Result<Signature, signature::Error>
    where
        T: Serialize,
    {
        let stored_key = self.keys.get(address).ok_or_else(|| {
            signature::Error::from_source(format!("Cannot find key for address: [{address}]"))
        })?;

        let intent_msg = &IntentMessage::new(intent, msg);
        match stored_key {
            StoredKey::KeyPair(keypair) => Ok(Signature::new_secure(intent_msg, keypair)),
            StoredKey::Account(_) => Err(signature::Error::from_source(
                "sign_secure is not supported for account type",
            )),
            StoredKey::External { source, .. } => Err(signature::Error::from_source(format!(
                "sign_secure is not supported for external type: {source} [{address}]",
            ))),
        }
    }

    fn add_key(
        &mut self,
        alias: Option<String>,
        key: impl Into<StoredKey>,
    ) -> Result<(), anyhow::Error> {
        let key = key.into();
        let address = key.address();

        let alias = self.create_alias(alias)?;
        self.aliases.insert(address, Alias { alias });
        self.keys.insert(address, key);
        self.save()?;
        Ok(())
    }

    fn remove_key(&mut self, address: &IotaAddress) -> Result<(), anyhow::Error> {
        self.aliases.remove(address);
        self.keys.remove(address);
        self.save()?;
        Ok(())
    }

    /// Return an array of `Alias`, consisting of every alias and its
    /// corresponding public key.
    fn aliases(&self) -> Vec<&Alias> {
        self.aliases.values().collect()
    }

    fn addresses_with_alias(&self) -> Vec<(&IotaAddress, &Alias)> {
        self.aliases.iter().collect::<Vec<_>>()
    }

    /// Return an array of `Alias`, consisting of every alias and its
    /// corresponding public key.
    fn aliases_mut(&mut self) -> Vec<&mut Alias> {
        self.aliases.values_mut().collect()
    }

    fn keys(&self) -> Vec<&StoredKey> {
        self.keys.values().collect()
    }

    /// This function returns an error if the provided alias already exists. If
    /// the alias has not already been used, then it returns the alias.
    /// If no alias has been passed, it will generate a new alias.
    fn create_alias(&self, alias: Option<String>) -> Result<String, anyhow::Error> {
        match alias {
            Some(a) if self.alias_exists(&a) => {
                bail!("Alias {a} already exists. Please choose another alias.")
            }
            Some(a) => validate_alias(&a),
            None => Ok(random_name(
                &self
                    .alias_names()
                    .into_iter()
                    .map(|x| x.to_string())
                    .collect::<HashSet<_>>(),
            )),
        }
    }

    /// Get the address by its alias
    fn get_address_by_alias(&self, alias: String) -> Result<&IotaAddress, anyhow::Error> {
        self.addresses_with_alias()
            .iter()
            .find(|x| x.1.alias == alias)
            .ok_or_else(|| anyhow!("Cannot resolve alias {alias} to an address"))
            .map(|x| x.0)
    }

    /// Get the alias if it exists, or return an error if it does not exist.
    fn get_alias_by_address(&self, address: &IotaAddress) -> Result<String, anyhow::Error> {
        match self.aliases.get(address) {
            Some(alias) => Ok(alias.alias.clone()),
            None => bail!("Cannot find alias for address {address}"),
        }
    }

    fn get_key(&self, address: &IotaAddress) -> Result<&StoredKey, anyhow::Error> {
        match self.keys.get(address) {
            Some(key) => Ok(key),
            None => Err(anyhow!("Cannot find key for address: [{address}]")),
        }
    }

    /// Updates an old alias to the new alias and saves it to the alias file.
    /// If the new_alias is None, it will generate a new random alias.
    fn update_alias(
        &mut self,
        old_alias: &str,
        new_alias: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        let new_alias_name = self.update_alias_value(old_alias, new_alias)?;
        self.save()?;
        Ok(new_alias_name)
    }
}

impl FileBasedKeystore {
    pub fn new_from_v1(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let keys = if path.exists() {
            let reader =
                BufReader::new(File::open(path).with_context(|| {
                    format!("Cannot open the keystore file: {}", path.display())
                })?);
            let kp_strings: Vec<String> = serde_json::from_reader(reader).with_context(|| {
                format!("Cannot deserialize the keystore file: {}", path.display(),)
            })?;
            kp_strings
                .iter()
                .map(|kpstr| {
                    let key = IotaKeyPair::decode(kpstr);
                    key.map(|k| (IotaAddress::from(&k.public()), StoredKey::KeyPair(k)))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map_err(|e| anyhow!("Invalid keystore file: {}. {}", path.display(), e))?
        } else {
            BTreeMap::new()
        };

        // check aliases
        let mut aliases_path = path.clone();
        aliases_path.set_extension("aliases");

        let aliases = if aliases_path.exists() {
            let reader = BufReader::new(File::open(&aliases_path).with_context(|| {
                format!(
                    "Cannot open aliases file in keystore: {}",
                    aliases_path.display()
                )
            })?);

            let legacy_aliases: Vec<LegacyAlias> =
                serde_json::from_reader(reader).with_context(|| {
                    format!(
                        "Cannot deserialize aliases file in keystore: {}",
                        aliases_path.display(),
                    )
                })?;

            legacy_aliases
                .into_iter()
                .map(|legacy_alias| {
                    let key = PublicKey::decode_base64(&legacy_alias.public_key_base64);
                    key.map(|k| {
                        (
                            Into::<IotaAddress>::into(&k),
                            Alias {
                                alias: legacy_alias.alias,
                            },
                        )
                    })
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map_err(|e| {
                    anyhow!(
                        "Invalid aliases file in keystore: {}. {}",
                        aliases_path.display(),
                        e
                    )
                })?
        } else if keys.is_empty() {
            BTreeMap::new()
        } else {
            let names: Vec<String> = random_names(HashSet::new(), keys.len());
            let aliases = keys
                .iter()
                .zip(names)
                .map(|((iota_address, _ikp), alias)| (*iota_address, Alias { alias }))
                .collect::<BTreeMap<_, _>>();
            let aliases_store = serde_json::to_string_pretty(&aliases.values().collect::<Vec<_>>())
                .with_context(|| {
                    format!(
                        "Cannot serialize aliases to file in keystore: {}",
                        aliases_path.display()
                    )
                })?;
            fs::write(aliases_path, aliases_store)?;
            aliases
        };

        Ok(Self {
            keys,
            aliases,
            path: path.to_path_buf(),
        })
    }

    fn needs_migration(path: &PathBuf) -> Result<bool, anyhow::Error> {
        let mut aliases_path = path.clone();
        aliases_path.set_extension("aliases");
        if aliases_path.exists() {
            // If the aliases file exists, we assume that the keystore is in v1 format
            debug!(
                "An alias file exists at {}, assuming keystore is in v1 format",
                aliases_path.display()
            );
            return Ok(true);
        }
        // If the aliases file does not exist, we check if the keystore file exists and
        // it has the old format
        if path.exists() {
            let reader =
                BufReader::new(File::open(path).with_context(|| {
                    format!("Cannot open the keystore file: {}", path.display())
                })?);
            // If we can deserialize the keystore file as a Vec<String>, it is in v1 format
            // If it fails, it is in v2 format or invalid
            let is_v1_format = serde_json::from_reader::<_, Vec<String>>(reader).is_ok();
            debug!(
                "Keystore file at {} is in v1 format: {}",
                path.display(),
                is_v1_format,
            );
            Ok(is_v1_format)
        } else {
            // If the keystore file does not exist, no migration is needed
            Ok(false)
        }
    }

    fn migrate_v1_to_v2(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let migrated = Self::new_from_v1(path)?;
        // If the migration was successful, we rename the <path> and <path>.aliases
        // files as a backup and append .migrated
        let mut backup_path = path.clone();
        backup_path.set_extension(
            // We append .migrated to the original file extension
            // add_extension is still experimental, so we do it manually
            path.extension()
                .and_then(|ext| Some(ext.to_str()?.to_owned() + ".migrated"))
                .unwrap_or(String::from("migrated")),
        );
        fs::rename(path, &backup_path).with_context(|| {
            format!(
                "Failed to rename the old keystore file to {}",
                backup_path.display()
            )
        })?;
        let mut aliases_path = path.clone();
        aliases_path.set_extension("aliases");
        let mut backup_aliases_path = aliases_path.clone();
        backup_aliases_path.set_extension("aliases.migrated");
        fs::rename(&aliases_path, &backup_aliases_path).with_context(|| {
            format!(
                "Failed to rename the old aliases file to {}",
                backup_aliases_path.display()
            )
        })?;

        info!(
            "Migrated {} keys in keystore from v1 to v2 format. Old files have been renamed to {} and {}",
            migrated.keys.len(),
            backup_path.display(),
            backup_aliases_path.display()
        );

        // Now we save the migrated keystore to the original path
        migrated.save()?;

        Ok(migrated)
    }

    pub fn new(path: &PathBuf) -> Result<Self, anyhow::Error> {
        if Self::needs_migration(path)? {
            return Self::migrate_v1_to_v2(path);
        }

        let (keys, aliases) = if path.exists() {
            let reader =
                BufReader::new(File::open(path).with_context(|| {
                    format!("Cannot open the keystore file: {}", path.display())
                })?);

            let file: FileBasedKeystoreFile = serde_json::from_reader(reader).map_err(|e| {
                anyhow!(
                    "Cannot deserialize the keystore file: {}. {e}",
                    path.display()
                )
            })?;

            let aliases = file
                .keys
                .iter()
                .map(|aliased| {
                    (
                        aliased.key.address(),
                        Alias {
                            alias: aliased.alias.clone(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();

            let keys = file
                .keys
                .into_iter()
                .map(|aliased| (aliased.key.address(), aliased.key))
                .collect::<BTreeMap<_, _>>();

            (keys, aliases)
        } else {
            (BTreeMap::new(), BTreeMap::new())
        };

        Ok(Self {
            keys,
            aliases,
            path: path.to_path_buf(),
        })
    }

    pub fn set_path(&mut self, path: &Path) {
        self.path = path.to_path_buf();
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        let file = FileBasedKeystoreFile {
            version: 2,
            keys: self
                .keys
                .iter()
                .map(|(address, key)| AliasedKey {
                    alias: self
                        .aliases
                        .get(address)
                        .map_or_else(|| self.create_alias(None).unwrap(), |a| a.alias.clone()),
                    address: *address,
                    key: key.clone(),
                })
                .collect(),
        };

        let store = serde_json::to_string_pretty(&file).with_context(|| {
            format!("Cannot serialize keystore to file: {}", self.path.display())
        })?;
        fs::write(&self.path, store)
            .map_err(|e| anyhow!("Couldn't save keystore to {}: {e}", self.path.display()))?;
        Ok(())
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct InMemKeystore {
    aliases: BTreeMap<IotaAddress, Alias>,
    keys: BTreeMap<IotaAddress, StoredKey>,
}

impl AccountKeystore for InMemKeystore {
    fn sign_hashed(
        &self,
        address: &IotaAddress,
        msg: &[u8],
    ) -> Result<Signature, signature::Error> {
        let stored_key = self.keys.get(address).ok_or_else(|| {
            signature::Error::from_source(format!("Cannot find key for address: [{address}]"))
        })?;

        match stored_key {
            StoredKey::KeyPair(keypair) => Ok(Signature::new_hashed(msg, keypair)),
            StoredKey::Account(_) => Err(signature::Error::from_source(
                "sign_hashed is not supported for account type",
            )),
            StoredKey::External { source, .. } => Err(signature::Error::from_source(format!(
                "sign_hashed is not supported for external type: {source} [{address}]"
            ))),
        }
    }
    fn sign_secure<T>(
        &self,
        address: &IotaAddress,
        msg: &T,
        intent: Intent,
    ) -> Result<Signature, signature::Error>
    where
        T: Serialize,
    {
        let stored_key = self.keys.get(address).ok_or_else(|| {
            signature::Error::from_source(format!("Cannot find key for address: [{address}]"))
        })?;

        let intent_msg = &IntentMessage::new(intent, msg);
        match stored_key {
            StoredKey::KeyPair(keypair) => Ok(Signature::new_secure(intent_msg, keypair)),
            StoredKey::Account(_) => Err(signature::Error::from_source(
                "sign_secure is not supported for account type",
            )),
            StoredKey::External { source, .. } => Err(signature::Error::from_source(format!(
                "sign_secure is not supported for external type: {source} [{address}]",
            ))),
        }
    }

    fn add_key(
        &mut self,
        alias: Option<String>,
        key: impl Into<StoredKey>,
    ) -> Result<(), anyhow::Error> {
        let key = key.into();
        let address: IotaAddress = (&key.public()).into();
        let alias = alias.unwrap_or_else(|| {
            random_name(
                &self
                    .aliases()
                    .iter()
                    .map(|x| x.alias.clone())
                    .collect::<HashSet<_>>(),
            )
        });

        let alias = Alias { alias };
        self.aliases.insert(address, alias);
        self.keys.insert(address, key);
        Ok(())
    }

    fn remove_key(&mut self, address: &IotaAddress) -> Result<(), anyhow::Error> {
        self.aliases.remove(address);
        self.keys.remove(address);
        Ok(())
    }

    /// Get all aliases objects
    fn aliases(&self) -> Vec<&Alias> {
        self.aliases.values().collect()
    }

    fn addresses_with_alias(&self) -> Vec<(&IotaAddress, &Alias)> {
        self.aliases.iter().collect::<Vec<_>>()
    }

    fn keys(&self) -> Vec<&StoredKey> {
        self.keys.values().collect()
    }

    fn get_key(&self, address: &IotaAddress) -> Result<&StoredKey, anyhow::Error> {
        match self.keys.get(address) {
            Some(key) => Ok(key),
            None => Err(anyhow!("Cannot find key for address: [{address}]")),
        }
    }

    /// Get alias of address
    fn get_alias_by_address(&self, address: &IotaAddress) -> Result<String, anyhow::Error> {
        match self.aliases.get(address) {
            Some(alias) => Ok(alias.alias.clone()),
            None => bail!("Cannot find alias for address {address}"),
        }
    }

    /// Get the address by its alias
    fn get_address_by_alias(&self, alias: String) -> Result<&IotaAddress, anyhow::Error> {
        self.addresses_with_alias()
            .iter()
            .find(|x| x.1.alias == alias)
            .ok_or_else(|| anyhow!("Cannot resolve alias {alias} to an address"))
            .map(|x| x.0)
    }

    /// This function returns an error if the provided alias already exists. If
    /// the alias has not already been used, then it returns the alias.
    /// If no alias has been passed, it will generate a new alias.
    fn create_alias(&self, alias: Option<String>) -> Result<String, anyhow::Error> {
        match alias {
            Some(a) if self.alias_exists(&a) => {
                bail!("Alias {a} already exists. Please choose another alias.")
            }
            Some(a) => validate_alias(&a),
            None => Ok(random_name(
                &self
                    .alias_names()
                    .into_iter()
                    .map(|x| x.to_string())
                    .collect::<HashSet<_>>(),
            )),
        }
    }

    fn aliases_mut(&mut self) -> Vec<&mut Alias> {
        self.aliases.values_mut().collect()
    }

    /// Updates an old alias to the new alias. If the new_alias is None,
    /// it will generate a new random alias.
    fn update_alias(
        &mut self,
        old_alias: &str,
        new_alias: Option<&str>,
    ) -> Result<String, anyhow::Error> {
        self.update_alias_value(old_alias, new_alias)
    }
}

impl InMemKeystore {
    pub fn new_insecure_for_tests(initial_key_number: usize) -> Self {
        let mut rng = StdRng::from_seed([0; 32]);
        let keys = (0..initial_key_number)
            .map(|_| get_key_pair_from_rng(&mut rng))
            .map(|(ad, k)| (ad, IotaKeyPair::Ed25519(k).into()))
            .collect::<BTreeMap<IotaAddress, StoredKey>>();

        let aliases = keys
            .iter()
            .zip(random_names(HashSet::new(), keys.len()))
            .map(|((iota_address, _ikp), alias)| (*iota_address, Alias { alias }))
            .collect::<BTreeMap<_, _>>();

        Self { aliases, keys }
    }
}

fn validate_alias(alias: &str) -> Result<String, anyhow::Error> {
    let re = Regex::new(r"^[A-Za-z][A-Za-z0-9-_\.]*$")
        .map_err(|_| anyhow!("Cannot build the regex needed to validate the alias naming"))?;
    let alias = alias.trim();
    ensure!(
        re.is_match(alias),
        "Invalid alias. A valid alias must start with a letter and can contain only letters, digits, hyphens (-), dots (.), or underscores (_)."
    );
    Ok(alias.to_string())
}

#[cfg(test)]
mod tests {
    use crate::keystore::validate_alias;

    #[test]
    fn validate_alias_test() {
        // OK
        assert!(validate_alias("A.B_dash").is_ok());
        assert!(validate_alias("A.B-C1_dash").is_ok());
        assert!(validate_alias("abc_123.iota").is_ok());
        // Not allowed
        assert!(validate_alias("A.B-C_dash!").is_err());
        assert!(validate_alias(".B-C_dash!").is_err());
        assert!(validate_alias("_test").is_err());
        assert!(validate_alias("123").is_err());
        assert!(validate_alias("@@123").is_err());
        assert!(validate_alias("@_Ab").is_err());
        assert!(validate_alias("_Ab").is_err());
        assert!(validate_alias("^A").is_err());
        assert!(validate_alias("-A").is_err());
    }
}
