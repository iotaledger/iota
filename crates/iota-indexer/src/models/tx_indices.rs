// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::{
    schema::{
        optimistic_tx_calls_fun, optimistic_tx_calls_mod, optimistic_tx_calls_pkg,
        optimistic_tx_changed_objects, optimistic_tx_input_objects, optimistic_tx_kinds,
        optimistic_tx_recipients, optimistic_tx_senders, tx_calls_fun, tx_calls_mod, tx_calls_pkg,
        tx_changed_objects, tx_digests, tx_input_objects, tx_kinds, tx_recipients, tx_senders,
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
    pub transaction_digest: Vec<u8>,
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

#[expect(clippy::type_complexity)]
impl TxIndex {
    pub fn split(
        self: TxIndex,
    ) -> (
        Vec<StoredTxSenders>,
        Vec<StoredTxRecipients>,
        Vec<StoredTxInputObject>,
        Vec<StoredTxChangedObject>,
        Vec<StoredTxPkg>,
        Vec<StoredTxMod>,
        Vec<StoredTxFun>,
        Vec<StoredTxDigest>,
        Vec<StoredTxKind>,
    ) {
        let tx_sequence_number = self.tx_sequence_number as i64;
        let tx_sender = StoredTxSenders {
            tx_sequence_number,
            sender: self.sender.to_vec(),
        };
        let tx_recipients = self
            .recipients
            .iter()
            .map(|s| StoredTxRecipients {
                tx_sequence_number,
                recipient: s.to_vec(),
                sender: self.sender.to_vec(),
            })
            .collect();
        let tx_input_objects = self
            .input_objects
            .iter()
            .map(|o| StoredTxInputObject {
                tx_sequence_number,
                object_id: bcs::to_bytes(&o).unwrap(),
                sender: self.sender.to_vec(),
            })
            .collect();
        let tx_changed_objects = self
            .changed_objects
            .iter()
            .map(|o| StoredTxChangedObject {
                tx_sequence_number,
                object_id: bcs::to_bytes(&o).unwrap(),
                sender: self.sender.to_vec(),
            })
            .collect();

        let mut packages = Vec::new();
        let mut packages_modules = Vec::new();
        let mut packages_modules_funcs = Vec::new();

        for (pkg, pkg_mod, pkg_mod_func) in self
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
                sender: self.sender.to_vec(),
            })
            .collect();

        let tx_mods = packages_modules
            .iter()
            .map(|(p, m)| StoredTxMod {
                tx_sequence_number,
                package: p.to_vec(),
                module: m.to_string(),
                sender: self.sender.to_vec(),
            })
            .collect();

        let tx_calls = packages_modules_funcs
            .iter()
            .map(|(p, m, f)| StoredTxFun {
                tx_sequence_number,
                package: p.to_vec(),
                module: m.to_string(),
                func: f.to_string(),
                sender: self.sender.to_vec(),
            })
            .collect();

        let stored_tx_digest = StoredTxDigest {
            tx_digest: self.transaction_digest.into_inner().to_vec(),
            tx_sequence_number,
        };

        let tx_kind = StoredTxKind {
            tx_kind: self.tx_kind as i16,
            tx_sequence_number,
        };

        (
            vec![tx_sender],
            tx_recipients,
            tx_input_objects,
            tx_changed_objects,
            tx_pkgs,
            tx_mods,
            tx_calls,
            vec![stored_tx_digest],
            vec![tx_kind],
        )
    }
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_senders)]
pub struct OptimisticTxSenders {
    pub tx_insertion_order: i64,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_recipients)]
pub struct OptimisticTxRecipients {
    pub tx_insertion_order: i64,
    pub recipient: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_input_objects)]
pub struct OptimisticTxInputObject {
    pub tx_insertion_order: i64,
    pub object_id: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_changed_objects)]
pub struct OptimisticTxChangedObject {
    pub tx_insertion_order: i64,
    pub object_id: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_calls_pkg)]
pub struct OptimisticTxPkg {
    pub tx_insertion_order: i64,
    pub package: Vec<u8>,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_calls_mod)]
pub struct OptimisticTxMod {
    pub tx_insertion_order: i64,
    pub package: Vec<u8>,
    pub module: String,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_calls_fun)]
pub struct OptimisticTxFun {
    pub tx_insertion_order: i64,
    pub package: Vec<u8>,
    pub module: String,
    pub func: String,
    pub sender: Vec<u8>,
}

#[derive(Queryable, Insertable, Selectable, Debug, Clone, Default)]
#[diesel(table_name = optimistic_tx_kinds)]
pub struct OptimisticTxKind {
    pub tx_kind: i16,
    pub tx_insertion_order: i64,
}

impl From<OptimisticTxSenders> for StoredTxSenders {
    fn from(tx_senders: OptimisticTxSenders) -> Self {
        StoredTxSenders {
            tx_sequence_number: tx_senders.tx_insertion_order,
            sender: tx_senders.sender,
        }
    }
}

impl From<StoredTxSenders> for OptimisticTxSenders {
    fn from(tx_senders: StoredTxSenders) -> Self {
        OptimisticTxSenders {
            tx_insertion_order: tx_senders.tx_sequence_number,
            sender: tx_senders.sender,
        }
    }
}

impl From<OptimisticTxRecipients> for StoredTxRecipients {
    fn from(tx_recipients: OptimisticTxRecipients) -> Self {
        StoredTxRecipients {
            tx_sequence_number: tx_recipients.tx_insertion_order,
            recipient: tx_recipients.recipient,
            sender: tx_recipients.sender,
        }
    }
}

impl From<StoredTxRecipients> for OptimisticTxRecipients {
    fn from(tx_recipients: StoredTxRecipients) -> Self {
        OptimisticTxRecipients {
            tx_insertion_order: tx_recipients.tx_sequence_number,
            recipient: tx_recipients.recipient,
            sender: tx_recipients.sender,
        }
    }
}

impl From<OptimisticTxInputObject> for StoredTxInputObject {
    fn from(tx_input_object: OptimisticTxInputObject) -> Self {
        StoredTxInputObject {
            tx_sequence_number: tx_input_object.tx_insertion_order,
            object_id: tx_input_object.object_id,
            sender: tx_input_object.sender,
        }
    }
}

impl From<StoredTxInputObject> for OptimisticTxInputObject {
    fn from(tx_input_object: StoredTxInputObject) -> Self {
        OptimisticTxInputObject {
            tx_insertion_order: tx_input_object.tx_sequence_number,
            object_id: tx_input_object.object_id,
            sender: tx_input_object.sender,
        }
    }
}

impl From<OptimisticTxChangedObject> for StoredTxChangedObject {
    fn from(tx_changed_object: OptimisticTxChangedObject) -> Self {
        StoredTxChangedObject {
            tx_sequence_number: tx_changed_object.tx_insertion_order,
            object_id: tx_changed_object.object_id,
            sender: tx_changed_object.sender,
        }
    }
}

impl From<StoredTxChangedObject> for OptimisticTxChangedObject {
    fn from(tx_changed_object: StoredTxChangedObject) -> Self {
        OptimisticTxChangedObject {
            tx_insertion_order: tx_changed_object.tx_sequence_number,
            object_id: tx_changed_object.object_id,
            sender: tx_changed_object.sender,
        }
    }
}

impl From<OptimisticTxPkg> for StoredTxPkg {
    fn from(tx_pkg: OptimisticTxPkg) -> Self {
        StoredTxPkg {
            tx_sequence_number: tx_pkg.tx_insertion_order,
            package: tx_pkg.package,
            sender: tx_pkg.sender,
        }
    }
}

impl From<StoredTxPkg> for OptimisticTxPkg {
    fn from(tx_pkg: StoredTxPkg) -> Self {
        OptimisticTxPkg {
            tx_insertion_order: tx_pkg.tx_sequence_number,
            package: tx_pkg.package,
            sender: tx_pkg.sender,
        }
    }
}

impl From<OptimisticTxMod> for StoredTxMod {
    fn from(tx_mod: OptimisticTxMod) -> Self {
        StoredTxMod {
            tx_sequence_number: tx_mod.tx_insertion_order,
            package: tx_mod.package,
            module: tx_mod.module,
            sender: tx_mod.sender,
        }
    }
}

impl From<StoredTxMod> for OptimisticTxMod {
    fn from(tx_mod: StoredTxMod) -> Self {
        OptimisticTxMod {
            tx_insertion_order: tx_mod.tx_sequence_number,
            package: tx_mod.package,
            module: tx_mod.module,
            sender: tx_mod.sender,
        }
    }
}

impl From<OptimisticTxFun> for StoredTxFun {
    fn from(tx_fun: OptimisticTxFun) -> Self {
        StoredTxFun {
            tx_sequence_number: tx_fun.tx_insertion_order,
            package: tx_fun.package,
            module: tx_fun.module,
            func: tx_fun.func,
            sender: tx_fun.sender,
        }
    }
}

impl From<StoredTxFun> for OptimisticTxFun {
    fn from(tx_fun: StoredTxFun) -> Self {
        OptimisticTxFun {
            tx_insertion_order: tx_fun.tx_sequence_number,
            package: tx_fun.package,
            module: tx_fun.module,
            func: tx_fun.func,
            sender: tx_fun.sender,
        }
    }
}

impl From<OptimisticTxKind> for StoredTxKind {
    fn from(tx_kind: OptimisticTxKind) -> Self {
        StoredTxKind {
            tx_kind: tx_kind.tx_kind,
            tx_sequence_number: tx_kind.tx_insertion_order,
        }
    }
}

impl From<StoredTxKind> for OptimisticTxKind {
    fn from(tx_kind: StoredTxKind) -> Self {
        OptimisticTxKind {
            tx_kind: tx_kind.tx_kind,
            tx_insertion_order: tx_kind.tx_sequence_number,
        }
    }
}
