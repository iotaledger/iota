// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;
use iota_grpc_client::GrpcClient;
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClient,
    rpc_types::{
        DevInspectArgs, DevInspectResults, IotaData, IotaExecutionStatus, IotaObjectData,
        IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery,
        IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse, ObjectChange,
    },
    wallet_context::WalletContext,
};
use iota_sdk_transaction_builder::{Receiving, SharedMut};
use iota_sdk_types::{
    Address, Identifier, MultisigAggregatedSignature, MultisigCommittee, ObjectId, ObjectReference,
    Owner, SharedObjectReference, StructTag, Transaction, TransactionKind,
    crypto::{Intent, UserSignature},
};
use iota_types::{
    crypto::PublicKey,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TransactionEnvelope},
};

use crate::{
    crypto::combine_keys,
    game::{self, Game, GameKind, Winner},
    turn_cap::TurnCap,
};

#[derive(Parser, Debug)]
pub struct Connection {
    /// The IOTA CLI config file, (default: ~/.iota/iota_config/client.yaml)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Object ID of the game's package.
    #[arg(long, short, env = "PKG")]
    package_id: ObjectId,
}

pub(crate) struct Client {
    wallet: WalletContext,
    package: ObjectId,
}

impl Client {
    /// Create a new client that derives its active address and RPC from the
    /// CLI's config (found at path `config`), and that expects to interact
    /// with the tic-tac-toe package at address `package`.
    pub(crate) fn new(conn: Connection) -> Result<Self> {
        let Some(config) = conn.config.or_else(|| {
            let mut default = dirs::home_dir()?;
            default.extend([".iota", "iota_config", "client.yaml"]);
            Some(default)
        }) else {
            bail!(
                "Cannot find wallet config. No config was supplied, and the default path \
                 (~/.iota/iota_config/client.yaml) does not exist.",
            );
        };

        let wallet = WalletContext::new(&config)?;
        Ok(Self {
            wallet,
            package: conn.package_id,
        })
    }

    /// Fetch the details of a game object from on-chain (can be either shared
    /// or owned).
    pub(crate) async fn game(&self, id: ObjectId) -> Result<Game> {
        let client = self.client().await?;

        // (1) Read from RPC
        let response = client
            .read_api()
            .get_object_with_options(
                id,
                IotaObjectDataOptions {
                    show_owner: true,
                    show_bcs: true,
                    ..Default::default()
                },
            )
            .await
            .context("Error fetching game over RPC.")?;

        if let Some(err) = response.error {
            bail!(err);
        }

        // (2) Perform validation checks
        let Some(IotaObjectData {
            object_id,
            version,
            digest,
            bcs: Some(raw),
            owner: Some(owner),
            ..
        }) = response.data
        else {
            bail!("INTERNAL ERROR: No data for game.");
        };

        let Some(raw) = raw.try_as_move() else {
            bail!("It is a package, not an object.");
        };

        if raw.struct_tag.name().as_str() != "Game" {
            bail!("It is not a Game object, it has type {}.", raw.struct_tag);
        }

        let package = ObjectId::new(raw.struct_tag.address().into_bytes());
        if package != self.package {
            bail!(
                "It is expected to be from package {} but is from package {}.",
                self.package,
                package,
            );
        }

        // (3) Deserialize contents
        let kind = match raw.struct_tag.module().as_str() {
            "shared" => GameKind::Shared(
                bcs::from_bytes(&raw.bcs_bytes).context("Failed to deserialize contents.")?,
            ),

            "owned" => GameKind::Owned(
                bcs::from_bytes(&raw.bcs_bytes).context("Failed to deserialize contents.")?,
            ),

            kind => bail!("{id} has unrecognised Game kind: {kind}."),
        };

        // (4) Check whether the game has ended or not.
        let mut builder = ProgrammableTransactionBuilder::new();
        let g = if let Owner::Shared(initial_shared_version) = owner {
            builder.obj(CallArg::Shared(SharedObjectReference::new(
                id,
                initial_shared_version,
                false,
            )))?
        } else {
            builder.obj(CallArg::ImmutableOrOwned(ObjectReference::new(
                object_id, version, digest,
            )))?
        };

        builder.programmable_move_call(
            self.package,
            raw.struct_tag.module().clone(),
            Identifier::from_static("ended"),
            vec![],
            vec![g],
        );

        let results = client
            .read_api()
            .dev_inspect_transaction_block(
                Address::ZERO,
                TransactionKind::Programmable(builder.finish()),
                None,
                None,
                Some(DevInspectArgs {
                    skip_checks: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .context("Error checking game winner.")?;

        fn extract_winner(results: &DevInspectResults) -> Option<Winner> {
            match *results
                .results
                .as_ref()?
                .first()?
                .return_values
                .first()?
                .0
                .first()?
            {
                0 => Some(Winner::None),
                1 => Some(Winner::Draw),
                2 => Some(Winner::Win),
                _ => None,
            }
        }

        let Some(winner) = extract_winner(&results) else {
            bail!("Error checking game winner.");
        };

        Ok(Game {
            kind,
            owner,
            version,
            digest,
            winner,
        })
    }

    /// Look for a `TurnCap` for the given `game` owned by the wallet's active
    /// address, and return its `ObjectReference`. Fails if no such `TurnCap`
    /// can be found.
    pub(crate) async fn turn_cap(&mut self, game: &Game) -> Result<ObjectReference> {
        let player = self.wallet.active_address()?;
        let client = self.client().await?;
        let game_id = game.object_ref().object_id;

        let turn_cap_type = StructTag::new(
            self.package,
            Identifier::from_static("owned"),
            Identifier::from_static("TurnCap"),
            vec![],
        );

        let query = Some(IotaObjectResponseQuery::new(
            Some(IotaObjectDataFilter::StructType(turn_cap_type.clone())),
            Some(IotaObjectDataOptions::new().with_bcs()),
        ));

        let mut cursor = None;
        loop {
            let response = client
                .read_api()
                .get_owned_objects(player, query.clone(), cursor, None)
                .await
                .context("Error fetching TurnCaps from RPC.")?;

            for IotaObjectResponse { data, error } in response.data {
                if let Some(err) = error {
                    bail!(err);
                }

                let Some(IotaObjectData {
                    object_id,
                    version,
                    digest,
                    bcs: Some(raw),
                    ..
                }) = data
                else {
                    continue;
                };

                let Some(raw) = raw.try_as_move() else {
                    continue;
                };

                if raw.struct_tag != turn_cap_type {
                    continue;
                }

                let turn_cap: TurnCap = bcs::from_bytes(&raw.bcs_bytes)
                    .context("INTERNAL ERROR: Failed to deserialize TurnCap.")?;

                if turn_cap.game == game_id {
                    return Ok(ObjectReference::new(object_id, version, digest));
                }
            }

            cursor = response.next_cursor;
            if !response.has_next_page {
                bail!("Could not find TurnCap. Is it your turn?");
            }
        }
    }

    /// Create a new shared game, between the wallet's active address and the
    /// given `opponent`. Returns the ID of the Game that was created on
    /// success.
    pub(crate) async fn new_shared_game(&mut self, opponent: Address) -> Result<ObjectId> {
        let player = self.wallet.active_address()?;

        let client = self.grpc_client().await?;
        let mut builder = client.transaction_builder(player);
        builder
            .move_call(self.package, "shared", "new")
            .arguments((player, opponent));

        let tx = builder.finish().await?;
        self.execute_for_game(tx).await
    }

    /// Create a new owned game, between the wallet's active address and the
    /// given `opponent`. The game is transferred to a 1-of-2 multisig
    /// address -- the admin -- where the two partial signatures are the
    /// player's and the opponent's.
    ///
    /// Returns the ID for the Game that was created on success.
    pub async fn new_owned_game(&mut self, opponent_key: PublicKey) -> Result<ObjectId> {
        let player = self.wallet.active_address()?;
        let player_key = self.wallet.config().keystore().get_key(&player)?.public();

        // The opponent's address can be derived from their public key, but not vice
        // versa.
        let opponent = Address::from(&opponent_key);

        // A 1-of-2 multisig acts as the admin of the game. The Game object will be
        // transferred to this address once it is created.
        let admin_key = combine_keys(vec![player_key, opponent_key])?;
        let admin = Address::from(&admin_key);
        let admin_bytes =
            bcs::to_bytes(&admin_key).context("INTERNAL ERROR: Failed to encode admin key.")?;

        let client = self.grpc_client().await?;
        let mut builder = client.transaction_builder(player);
        let game = builder
            .move_call(self.package, "owned", "new")
            .arguments((player, opponent, admin_bytes))
            .result();
        builder.transfer_objects(admin, [game]);

        let tx = builder.finish().await?;
        self.execute_for_game(tx).await
    }

    /// Delete a shared game, given itself contents and its ownership
    /// information (which should be a `Owner::Shared`).
    pub async fn delete_shared_game(&mut self, game: &game::Shared, owner: Owner) -> Result<()> {
        let player = self.wallet.active_address()?;

        let Owner::Shared(_) = owner else {
            bail!("Game is not shared");
        };

        let client = self.grpc_client().await?;
        let mut builder = client.transaction_builder(player);
        builder
            .move_call(self.package, "shared", "burn")
            .arguments([SharedMut(game.board.id)]);

        let data = builder.finish().await?;
        let tx = self.wallet.sign_transaction(&data);
        self.execute_transaction(tx).await?;
        Ok(())
    }

    /// Delete an owned (multi-sig) game. The transaction is signed by the
    /// player on behalf of the admin (multi-sig) address, and also directly
    /// by the player who is acting as the sponsor.
    pub async fn delete_owned_game(
        &mut self,
        game: &game::Owned,
        game_ref: ObjectReference,
    ) -> Result<()> {
        let player = self.wallet.active_address()?;

        let admin_key: MultisigCommittee =
            bcs::from_bytes(&game.admin).context("Failed to deserialize admin's public key.")?;
        let admin = Address::from(&admin_key);

        let client = self.grpc_client().await?;
        let mut builder = client.transaction_builder(admin);
        builder
            .move_call(self.package, "owned", "burn")
            .arguments([game_ref]);

        builder.sponsor(player);
        let data = builder.finish().await?;

        let tx = self
            .multi_sig_transaction(player, admin_key, data)
            .await
            .context("Failed multi-sign transaction.")?;

        self.execute_transaction(tx).await?;
        Ok(())
    }

    /// Make a move on a shared game as the wallet's active address. Fails if
    /// the active address is not meant to make the next move, or if the
    /// position is already occupied.
    pub async fn make_shared_move(
        &mut self,
        game: &game::Shared,
        owner: Owner,
        row: u8,
        col: u8,
    ) -> Result<()> {
        let player = self.wallet.active_address()?;

        let Owner::Shared(_) = owner else {
            bail!("Game is not shared");
        };

        let client = self.grpc_client().await?;
        let mut builder = client.transaction_builder(player);
        builder
            .move_call(self.package, "shared", "place_mark")
            .arguments((SharedMut(game.board.id), row, col));

        let data = builder.finish().await?;
        let tx = self.wallet.sign_transaction(&data);
        self.execute_transaction(tx).await?;
        Ok(())
    }

    /// Make a move on an owned game as the wallet's active address. This
    /// involves sending two transactions: The first from the player to
    /// create a `Mark`, and a second from the admin to receive the mark and
    /// apply it.
    pub async fn make_owned_move(
        &mut self,
        game: &game::Owned,
        game_ref: ObjectReference,
        cap_ref: ObjectReference,
        row: u8,
        col: u8,
    ) -> Result<()> {
        let player = self.wallet.active_address()?;

        // First transaction sends the mark to the game.
        let client = self.grpc_client().await?;
        let mut builder = client.transaction_builder(player);
        builder
            .move_call(self.package, "owned", "send_mark")
            .arguments((cap_ref, row, col));

        let data = builder.finish().await?;
        let tx = self.wallet.sign_transaction(&data);
        let IotaTransactionBlockResponse {
            object_changes: Some(object_changes),
            ..
        } = self
            .execute_transaction(tx)
            .await
            .context("Failed to send mark.")?
        else {
            bail!("Can't find Mark.");
        };

        let Some(mark) = object_changes.into_iter().find_map(|change| {
            let ObjectChange::Created {
                object_type,
                object_id,
                version,
                digest,
                ..
            } = change
            else {
                return None;
            };

            if object_type.address().as_bytes() != self.package.as_bytes() {
                return None;
            }

            if object_type.name().as_str() != "Mark" {
                return None;
            }

            Some(ObjectReference::new(object_id, version, digest))
        }) else {
            bail!("Can't find Mark");
        };

        // Second transaction applies the mark to the game, and needs to be run as the
        // admin.
        let admin_key: MultisigCommittee =
            bcs::from_bytes(&game.admin).context("Failed to deserialize admin's public key.")?;
        let admin = Address::from(&admin_key);

        let mut builder = client.transaction_builder(admin);
        builder
            .move_call(self.package, "owned", "place_mark")
            .arguments((game_ref, Receiving(mark)));

        builder.sponsor(player);
        let data = builder.finish().await?;

        let tx = self
            .multi_sig_transaction(player, admin_key, data)
            .await
            .context("Failed multi-sign transaction.")?;

        self.execute_transaction(tx)
            .await
            .context("Failed to place mark.")?;

        Ok(())
    }

    /// Execute a PTB, expecting it to create a shared or owned Game, and return
    /// its ObjectId.
    async fn execute_for_game(&self, tx: Transaction) -> Result<ObjectId> {
        let tx = self.wallet.sign_transaction(&tx);
        let IotaTransactionBlockResponse {
            object_changes: Some(object_changes),
            ..
        } = self.execute_transaction(tx).await?
        else {
            bail!("Can't find Game ID");
        };

        let Some(game_id) = object_changes.into_iter().find_map(|change| {
            let ObjectChange::Created {
                object_type,
                object_id,
                ..
            } = change
            else {
                return None;
            };

            if object_type.address().as_bytes() != self.package.as_bytes() {
                return None;
            }

            if object_type.name().as_str() != "Game" {
                return None;
            }

            Some(object_id)
        }) else {
            bail!("Can't find Game ID");
        };

        Ok(game_id)
    }

    /// The wallet's gRPC client.
    async fn grpc_client(&self) -> Result<GrpcClient> {
        self.wallet
            .get_grpc_client()
            .await
            .context("Error fetching gRPC client")
    }

    /// Sign the transaction as `sender` by itself (as the sponsor) and as part
    /// of the multi-sig, `admin_key` (the transaction sender), and execute
    /// it.
    async fn multi_sig_transaction(
        &self,
        sender: Address,
        admin_key: MultisigCommittee,
        tx: Transaction,
    ) -> Result<TransactionEnvelope> {
        let sponsor_sig: UserSignature = self
            .wallet
            .config()
            .keystore()
            .sign_secure(&sender, &tx, Intent::iota_transaction())
            .context("Signing transaction")?
            .into();

        let multi_sig: UserSignature =
            MultisigAggregatedSignature::new(vec![sponsor_sig.clone()], admin_key)
                .context("Signing as admin")?
                .into();

        Ok(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![multi_sig, sponsor_sig],
        ))
    }

    /// Execute the transaction, and check whether it succeeded or failed.
    /// Transaction execution failure is treated as an error.
    async fn execute_transaction(
        &self,
        tx: TransactionEnvelope,
    ) -> Result<IotaTransactionBlockResponse> {
        let response = self
            .wallet
            .execute_transaction_may_fail(tx)
            .await
            .context("Error executing transaction")?;

        let Some(effects) = &response.effects else {
            bail!("Failed to find effects for transaction");
        };

        if let IotaExecutionStatus::Failure { error } = effects.status() {
            bail!(error.to_owned());
        }

        Ok(response)
    }

    async fn client(&self) -> Result<IotaClient> {
        self.wallet
            .get_client()
            .await
            .context("Error fetching client")
    }
}
