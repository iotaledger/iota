// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::{
    schema::{
        tx_calls_fun, tx_calls_mod, tx_calls_pkg, tx_changed_objects, tx_digests, tx_input_objects,
        tx_kinds, tx_recipients, tx_senders, tx_wrapped_or_deleted_objects,
    },
    types::TxIndex,
};

#[derive(QueryableByName)]
pub struct TxSequenceNumber {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub tx_sequence_number: i64,
}

#[derive(QueryableByName)]
pub struct TxDigest {
    #[diesel(sql_type = diesel::sql_types::Binary)]
    pub tx_digest: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_senders)]
pub struct StoredTxSenders {
    pub tx_sequence_number: i64,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_recipients)]
pub struct StoredTxRecipients {
    pub tx_sequence_number: i64,
    pub recipient: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_input_objects)]
pub struct StoredTxInputObject {
    pub tx_sequence_number: i64,
    pub object_id: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_changed_objects)]
pub struct StoredTxChangedObject {
    pub tx_sequence_number: i64,
    pub object_id: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_wrapped_or_deleted_objects)]
pub struct StoredTxWrappedOrDeletedObject {
    pub tx_sequence_number: i64,
    pub object_id: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_calls_pkg)]
pub struct StoredTxPkg {
    pub tx_sequence_number: i64,
    pub package: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_calls_mod)]
pub struct StoredTxMod {
    pub tx_sequence_number: i64,
    pub package: Vec<u8>,
    pub module: String,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_calls_fun)]
pub struct StoredTxFun {
    pub tx_sequence_number: i64,
    pub package: Vec<u8>,
    pub module: String,
    pub func: String,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_digests)]
pub struct StoredTxDigest {
    pub tx_digest: Vec<u8>,
    pub tx_sequence_number: i64,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = tx_kinds)]
pub struct StoredTxKind {
    pub tx_kind: i16,
    pub tx_sequence_number: i64,
}

impl From<TxIndex> for TxIndexSplit {
    fn from(value: TxIndex) -> Self {
        let tx_wrapped_or_deleted_objects = value
            .wrapped_or_deleted_objects
            .into_iter()
            .map(|o| StoredTxWrappedOrDeletedObject {
                tx_sequence_number: value.tx_sequence_number as i64,
                object_id: bcs::to_bytes(&o)
                    .expect("indexed object should serialize without issues"),
                sender: value.sender.to_vec(),
            })
            .collect();

        let tx_sequence_number = value.tx_sequence_number as i64;
        let tx_sender = StoredTxSenders {
            tx_sequence_number,
            sender: value.sender.to_vec(),
        };
        let tx_recipients = value
            .recipients
            .iter()
            .map(|s| StoredTxRecipients {
                tx_sequence_number,
                recipient: s.to_vec(),
                sender: value.sender.to_vec(),
            })
            .collect();
        let tx_input_objects = value
            .input_objects
            .iter()
            .map(|o| StoredTxInputObject {
                tx_sequence_number,
                object_id: bcs::to_bytes(&o)
                    .expect("indexed object should serialize without issues"),
                sender: value.sender.to_vec(),
            })
            .collect();
        let tx_changed_objects = value
            .changed_objects
            .iter()
            .map(|o| StoredTxChangedObject {
                tx_sequence_number,
                object_id: bcs::to_bytes(&o)
                    .expect("indexed object should serialize without issues"),
                sender: value.sender.to_vec(),
            })
            .collect();

        let mut packages = Vec::new();
        let mut packages_modules = Vec::new();
        let mut packages_modules_funcs = Vec::new();

        for (pkg, pkg_mod, pkg_mod_func) in value
            .move_calls
            .iter()
            .map(|(p, m, f)| (*p, (*p, m.clone()), (*p, m.clone(), f.clone())))
        {
            packages.push(pkg);
            packages_modules.push(pkg_mod);
            packages_modules_funcs.push(pkg_mod_func);
        }

        let tx_pkgs = packages
            .iter()
            .map(|p| StoredTxPkg {
                tx_sequence_number,
                package: p.to_vec(),
                sender: value.sender.to_vec(),
            })
            .collect();

        let tx_mods = packages_modules
            .iter()
            .map(|(p, m)| StoredTxMod {
                tx_sequence_number,
                package: p.to_vec(),
                module: m.to_string(),
                sender: value.sender.to_vec(),
            })
            .collect();

        let tx_funs = packages_modules_funcs
            .iter()
            .map(|(p, m, f)| StoredTxFun {
                tx_sequence_number,
                package: p.to_vec(),
                module: m.to_string(),
                func: f.to_string(),
                sender: value.sender.to_vec(),
            })
            .collect();

        let stored_tx_digest = StoredTxDigest {
            tx_digest: value.transaction_digest.into_inner().to_vec(),
            tx_sequence_number,
        };

        let tx_kind = StoredTxKind {
            tx_kind: value.tx_kind as i16,
            tx_sequence_number,
        };

        let tx_senders = vec![tx_sender];
        let tx_digests = vec![stored_tx_digest];
        let tx_kinds = vec![tx_kind];
        Self {
            tx_senders,
            tx_recipients,
            tx_input_objects,
            tx_changed_objects,
            tx_wrapped_or_deleted_objects,
            tx_pkgs,
            tx_mods,
            tx_funs,
            tx_digests,
            tx_kinds,
        }
    }
}

pub(crate) struct TxIndexSplit {
    pub(crate) tx_senders: Vec<StoredTxSenders>,
    pub(crate) tx_recipients: Vec<StoredTxRecipients>,
    pub(crate) tx_input_objects: Vec<StoredTxInputObject>,
    pub(crate) tx_changed_objects: Vec<StoredTxChangedObject>,
    pub(crate) tx_wrapped_or_deleted_objects: Vec<StoredTxWrappedOrDeletedObject>,
    pub(crate) tx_pkgs: Vec<StoredTxPkg>,
    pub(crate) tx_mods: Vec<StoredTxMod>,
    pub(crate) tx_funs: Vec<StoredTxFun>,
    pub(crate) tx_digests: Vec<StoredTxDigest>,
    pub(crate) tx_kinds: Vec<StoredTxKind>,
}
