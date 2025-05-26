// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_names::config::IotaNamesConfig;
use iota_types::{
    Identifier,
    effects::{TransactionEffects, TransactionEffectsAPI},
    execution_status::ExecutionStatus,
    full_checkpoint_content::CheckpointData,
};

use crate::metrics::METRICS;

pub(crate) struct IotaNamesWorker;

#[async_trait]
impl Worker for IotaNamesWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>, // TODO change to &?
    ) -> Result<Self::Message, Self::Error> {
        println!(
            "Processing checkpoint: {}",
            checkpoint.checkpoint_summary.sequence_number
        );
        let config = IotaNamesConfig::from_env().unwrap_or_default();

        // let mut num_registrations = 0;
        for transaction in &checkpoint.transactions {
            let TransactionEffects::V1(effects) = &transaction.effects;

            if *effects.status() != ExecutionStatus::Success {
                continue;
            }

            if let Some(events) = &transaction.events {
                for event in events.data.iter() {
                    println!("Event: {event:#?}");
                    if event.type_.address == config.package_address.into() {
                        println!(
                            "Event for tx {} in checkpoint {}: {event:#?}",
                            transaction.transaction.digest(),
                            checkpoint.checkpoint_summary.sequence_number
                        );
                        if event.type_.name == Identifier::new("IotaNamesRegistryEvent")? {
                            // TODO: init from prometheus storage to not always start from 0
                            METRICS
                                .get()
                                .expect("metrics global should be initialized")
                                .total_name_records
                                .add(1);
                            // TODO: deserialize to get the name lengths
                            // let register_event =
                            //     bcs::from_bytes::<IotaNamesRegistryEvent>(&
                            // event_bcs_bytes)?;
                            // println!("Register event: {register_event:#?}");
                        }
                    }
                }
            }
            // let TransactionData::V1(data) =
            // &transaction.transaction.intent_message().value;
            // let module = Identifier::new("payment")?; // TODO: Make const
            // let function = Identifier::new("register")?;

            // match &data.kind {
            //     TransactionKind::ProgrammableTransaction(txn) => {
            //         // println!("{txn:?}");
            //         if txn.commands.iter().any(|cmd| {
            //             if let Command::MoveCall(call) = cmd {
            //                 call.package ==
            // ObjectID::from(config.package_address)
            // && call.module == module                     &&
            // call.function == function             } else {
            //                 false
            //             }
            //         }) {
            //             num_registrations += 1;
            //         }
            //     }
            //     _ => (),
            // }
        }
        // if num_registrations != 0 {
        //     println!(
        //         "Registered {num_registrations} names in checkpoint {}",
        //         checkpoint.checkpoint_summary.sequence_number
        //     );
        // }

        Ok(())
    }
}
