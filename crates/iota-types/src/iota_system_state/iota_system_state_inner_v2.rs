// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use fastcrypto::traits::ToFromBytes;
use serde::{Deserialize, Serialize};

use super::iota_system_state_inner_v1::VerifiedValidatorMetadataV1;
use crate::{
    base_types::IotaAddress,
    collection_types::Bag,
    crypto::{
        AuthorityPublicKey, AuthoritySignature, NetworkPublicKey, verify_proof_of_possession,
    },
    multiaddr::Multiaddr,
};

const E_METADATA_INVALID_POP: u64 = 0;
const E_METADATA_INVALID_AUTHORITY_PUBKEY: u64 = 1;
const E_METADATA_INVALID_NET_PUBKEY: u64 = 2;
const E_METADATA_INVALID_PROTOCOL_PUBKEY: u64 = 3;
const E_METADATA_INVALID_NET_ADDR: u64 = 4;
const E_METADATA_INVALID_P2P_ADDR: u64 = 5;
const E_METADATA_INVALID_PRIMARY_ADDR: u64 = 6;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ValidatorMetadataV2 {
    pub iota_address: IotaAddress,
    pub authority_pubkey_bytes: Vec<u8>,
    pub network_pubkey_bytes: Vec<u8>,
    pub protocol_pubkey_bytes: Vec<u8>,
    pub proof_of_possession_bytes: Vec<u8>,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub project_url: String,
    pub net_address: String,
    pub p2p_address: String,
    pub primary_address: String,
    pub next_epoch_authority_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_proof_of_possession: Option<Vec<u8>>,
    pub next_epoch_network_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_protocol_pubkey_bytes: Option<Vec<u8>>,
    pub next_epoch_net_address: Option<String>,
    pub next_epoch_p2p_address: Option<String>,
    pub next_epoch_primary_address: Option<String>,
    pub extra_fields: Bag,
}

impl ValidatorMetadataV2 {
    /// Verify validator metadata and return a verified version (on success) or
    /// error code (on failure)
    /// V2: primary_address, next_epoch_primary_address are enforced to be TCP
    pub fn verify(&self) -> Result<VerifiedValidatorMetadataV1, u64> {
        let authority_pubkey = AuthorityPublicKey::from_bytes(self.authority_pubkey_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_AUTHORITY_PUBKEY)?;

        // Verify proof of possession for the authority key
        let pop = AuthoritySignature::from_bytes(self.proof_of_possession_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_POP)?;
        verify_proof_of_possession(&pop, &authority_pubkey, self.iota_address)
            .map_err(|_| E_METADATA_INVALID_POP)?;

        let network_pubkey = NetworkPublicKey::from_bytes(self.network_pubkey_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_NET_PUBKEY)?;
        let protocol_pubkey = NetworkPublicKey::from_bytes(self.protocol_pubkey_bytes.as_ref())
            .map_err(|_| E_METADATA_INVALID_PROTOCOL_PUBKEY)?;
        if protocol_pubkey == network_pubkey {
            return Err(E_METADATA_INVALID_PROTOCOL_PUBKEY);
        }

        let net_address = Multiaddr::try_from(self.net_address.clone())
            .map_err(|_| E_METADATA_INVALID_NET_ADDR)?;

        // Ensure p2p and primary address are both Multiaddr's and valid
        // anemo addresses
        let p2p_address = Multiaddr::try_from(self.p2p_address.clone())
            .map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;
        p2p_address
            .to_anemo_address()
            .map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;

        let primary_address = Multiaddr::try_from(self.primary_address.clone())
            .map_err(|_| E_METADATA_INVALID_PRIMARY_ADDR)?;
        if !primary_address.is_loosely_valid_tcp_addr() {
            return Err(E_METADATA_INVALID_PRIMARY_ADDR);
        }

        let next_epoch_authority_pubkey = match self.next_epoch_authority_pubkey_bytes.clone() {
            None => Ok::<Option<AuthorityPublicKey>, u64>(None),
            Some(bytes) => Ok(Some(
                AuthorityPublicKey::from_bytes(bytes.as_ref())
                    .map_err(|_| E_METADATA_INVALID_AUTHORITY_PUBKEY)?,
            )),
        }?;

        let next_epoch_pop = match self.next_epoch_proof_of_possession.clone() {
            None => Ok::<Option<AuthoritySignature>, u64>(None),
            Some(bytes) => Ok(Some(
                AuthoritySignature::from_bytes(bytes.as_ref())
                    .map_err(|_| E_METADATA_INVALID_POP)?,
            )),
        }?;
        // Verify proof of possession for the next epoch authority key
        if let Some(ref next_epoch_authority_pubkey) = next_epoch_authority_pubkey {
            match next_epoch_pop {
                Some(next_epoch_pop) => {
                    verify_proof_of_possession(
                        &next_epoch_pop,
                        next_epoch_authority_pubkey,
                        self.iota_address,
                    )
                    .map_err(|_| E_METADATA_INVALID_POP)?;
                }
                None => {
                    return Err(E_METADATA_INVALID_POP);
                }
            }
        }

        let next_epoch_network_pubkey = match self.next_epoch_network_pubkey_bytes.clone() {
            None => Ok::<Option<NetworkPublicKey>, u64>(None),
            Some(bytes) => Ok(Some(
                NetworkPublicKey::from_bytes(bytes.as_ref())
                    .map_err(|_| E_METADATA_INVALID_NET_PUBKEY)?,
            )),
        }?;

        let next_epoch_protocol_pubkey: Option<NetworkPublicKey> =
            match self.next_epoch_protocol_pubkey_bytes.clone() {
                None => Ok::<Option<NetworkPublicKey>, u64>(None),
                Some(bytes) => Ok(Some(
                    NetworkPublicKey::from_bytes(bytes.as_ref())
                        .map_err(|_| E_METADATA_INVALID_PROTOCOL_PUBKEY)?,
                )),
            }?;
        if next_epoch_network_pubkey.is_some()
            && next_epoch_network_pubkey == next_epoch_protocol_pubkey
        {
            return Err(E_METADATA_INVALID_PROTOCOL_PUBKEY);
        }

        let next_epoch_net_address = match self.next_epoch_net_address.clone() {
            None => Ok::<Option<Multiaddr>, u64>(None),
            Some(address) => Ok(Some(
                Multiaddr::try_from(address).map_err(|_| E_METADATA_INVALID_NET_ADDR)?,
            )),
        }?;

        let next_epoch_p2p_address = match self.next_epoch_p2p_address.clone() {
            None => Ok::<Option<Multiaddr>, u64>(None),
            Some(address) => {
                let address =
                    Multiaddr::try_from(address).map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;
                address
                    .to_anemo_address()
                    .map_err(|_| E_METADATA_INVALID_P2P_ADDR)?;

                Ok(Some(address))
            }
        }?;

        let next_epoch_primary_address = match self.next_epoch_primary_address.clone() {
            None => Ok::<Option<Multiaddr>, u64>(None),
            Some(address) => {
                let address =
                    Multiaddr::try_from(address).map_err(|_| E_METADATA_INVALID_PRIMARY_ADDR)?;
                if !address.is_loosely_valid_tcp_addr() {
                    return Err(E_METADATA_INVALID_PRIMARY_ADDR);
                };

                Ok(Some(address))
            }
        }?;

        Ok(VerifiedValidatorMetadataV1 {
            iota_address: self.iota_address,
            authority_pubkey,
            network_pubkey,
            protocol_pubkey,
            proof_of_possession_bytes: self.proof_of_possession_bytes.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            image_url: self.image_url.clone(),
            project_url: self.project_url.clone(),
            net_address,
            p2p_address,
            primary_address,
            next_epoch_authority_pubkey,
            next_epoch_proof_of_possession: self.next_epoch_proof_of_possession.clone(),
            next_epoch_network_pubkey,
            next_epoch_protocol_pubkey,
            next_epoch_net_address,
            next_epoch_p2p_address,
            next_epoch_primary_address,
        })
    }
}

#[cfg(test)]
mod tests {
    use fastcrypto::{
        bls12381::min_sig::BLS12381KeyPair,
        ed25519::Ed25519KeyPair,
        traits::{KeyPair, ToFromBytes},
    };
    use rand::{SeedableRng as _, rngs::StdRng};

    use super::ValidatorMetadataV2;
    use crate::crypto::generate_proof_of_possession;

    fn validator_for_testing(
        primary_address: &str,
        next_epoch_primary_address: Option<String>,
    ) -> ValidatorMetadataV2 {
        let mut rng = StdRng::from_seed([0; 32]);
        let authority_key_pair = BLS12381KeyPair::generate(&mut rng);
        let proof_of_possession =
            generate_proof_of_possession(&authority_key_pair, authority_key_pair.public().into());
        let network_key_pair = Ed25519KeyPair::generate(&mut rng);
        let protocol_key_pair = Ed25519KeyPair::generate(&mut rng);

        ValidatorMetadataV2 {
            iota_address: authority_key_pair.public().into(),
            authority_pubkey_bytes: authority_key_pair.public().as_bytes().to_vec(),
            network_pubkey_bytes: network_key_pair.public().as_bytes().to_vec(),
            protocol_pubkey_bytes: protocol_key_pair.public().as_bytes().to_vec(),
            proof_of_possession_bytes: proof_of_possession.as_bytes().to_vec(),
            name: String::new(),
            description: String::new(),
            image_url: String::new(),
            project_url: String::new(),
            net_address: "/ip4/127.0.0.1/tcp/8080".to_string(),
            p2p_address: "/ip4/127.0.0.1/udp/8080".to_string(),
            primary_address: primary_address.to_string(),
            next_epoch_authority_pubkey_bytes: None,
            next_epoch_proof_of_possession: None,
            next_epoch_network_pubkey_bytes: None,
            next_epoch_protocol_pubkey_bytes: None,
            next_epoch_net_address: None,
            next_epoch_p2p_address: None,
            next_epoch_primary_address,
            extra_fields: Default::default(),
        }
    }

    #[test]
    fn validator_with_udp_primary_addr() {
        let validator = validator_for_testing("/ip4/127.0.0.1/udp/8080", None);
        assert!(validator.verify().is_err());
    }

    #[test]
    fn validator_with_udp_next_epoch_primary_addr() {
        let validator = validator_for_testing(
            "/ip4/127.0.0.1/tcp/8080",
            Some("/ip4/127.0.0.1/udp/8080".to_string()),
        );
        assert!(validator.verify().is_err());
    }

    #[test]
    fn validator_with_tcp_primary_addr() {
        let validator = validator_for_testing("/ip4/127.0.0.1/tcp/8080", None);
        assert!(validator.verify().is_ok());
    }

    #[test]
    fn validator_with_tcp_next_epoch_primary_addr() {
        let validator = validator_for_testing(
            "/ip4/127.0.0.1/tcp/8080",
            Some("/ip4/127.0.0.1/tcp/8080".to_string()),
        );
        assert!(validator.verify().is_ok());
    }
}
