// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter};

use iota_grpc_types::v1::transaction::{
    ExecutedTransaction, object_change::Kind as ObjectChangeKind,
};
use iota_sdk_types::ObjectReference;
use iota_types::effects::TransactionEffectsAPI;
use tabled::{
    builder::Builder as TableBuilder,
    settings::{Panel as TablePanel, Style as TableStyle, style::HorizontalLine},
};

use crate::displays::Pretty;

impl Display for Pretty<'_, ExecutedTransaction> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Pretty(executed) = self;

        if let Ok(digest) = executed.transaction().and_then(|t| t.digest()) {
            writeln!(f, "Transaction Digest: {digest}")?;
        }

        if let Ok(effects) = executed.effects().and_then(|e| e.effects()) {
            let mut builder = TableBuilder::default();
            builder.push_record(vec![format!("Status: {:?}", effects.status())]);
            builder.push_record(vec![format!("{}", Pretty(effects.gas_cost_summary()))]);

            let created = effects.created();
            if !created.is_empty() {
                builder.push_record(vec!["\nCreated Objects:".to_string()]);
                for oref in &created {
                    builder.push_record(vec![format!(
                        "{}\n   Owner: {}",
                        objref_string(oref.reference()),
                        oref.owner()
                    )]);
                }
            }

            let mutated = effects.mutated();
            if !mutated.is_empty() {
                builder.push_record(vec!["\nMutated Objects:".to_string()]);
                for oref in &mutated {
                    builder.push_record(vec![format!(
                        "{}\n   Owner: {}",
                        objref_string(oref.reference()),
                        oref.owner()
                    )]);
                }
            }

            let deleted = effects.deleted();
            if !deleted.is_empty() {
                builder.push_record(vec!["\nDeleted Objects:".to_string()]);
                for oref in &deleted {
                    builder.push_record(vec![objref_string(oref)]);
                }
            }

            let mut table = builder.build();
            table.with(TablePanel::header("Transaction Effects"));
            table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
                1,
                TableStyle::modern().get_horizontal(),
            )]));
            writeln!(f, "{table}")?;
        }

        if let Ok(events) = executed.events().and_then(|e| e.events()) {
            if !events.0.is_empty() {
                let mut builder = TableBuilder::default();
                for event in &events.0 {
                    builder.push_record(vec![format!(
                        " ┌──\n │ EventType: {}\n │ PackageID: {}\n │ Sender: {}\n └──",
                        event.struct_tag, event.package_id, event.sender,
                    )]);
                }
                let mut table = builder.build();
                table.with(TablePanel::header("Events"));
                table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
                    1,
                    TableStyle::modern().get_horizontal(),
                )]));
                writeln!(f, "{table}")?;
            }
        }

        if let Ok(object_changes) = executed.object_changes() {
            let mut builder = TableBuilder::default();
            let (mut created, mut deleted, mut mutated, mut published, mut wrapped, mut unwrapped) =
                (vec![], vec![], vec![], vec![], vec![], vec![]);

            for change in &object_changes.object_changes {
                match &change.kind {
                    Some(ObjectChangeKind::Created(c)) => created.push(format!(
                        " ┌──\n │ ObjectID: {}\n │ Sender: {}\n │ Owner: {}\n │ ObjectType: {}\n │ Version: {}\n │ Digest: {}\n └──",
                        opt(c.object_id()),
                        opt(c.sender()),
                        opt(c.owner()),
                        opt(c.object_type()),
                        opt_u64(c.version),
                        opt(c.digest()),
                    )),
                    Some(ObjectChangeKind::Mutated(c)) => mutated.push(format!(
                        " ┌──\n │ ObjectID: {}\n │ Sender: {}\n │ Owner: {}\n │ ObjectType: {}\n │ Version: {}\n │ Digest: {}\n └──",
                        opt(c.object_id()),
                        opt(c.sender()),
                        opt(c.owner()),
                        opt(c.object_type()),
                        opt_u64(c.version),
                        opt(c.digest()),
                    )),
                    Some(ObjectChangeKind::Deleted(c)) => deleted.push(format!(
                        " ┌──\n │ ObjectID: {}\n │ Sender: {}\n │ ObjectType: {}\n │ Version: {}\n └──",
                        opt(c.object_id()),
                        opt(c.sender()),
                        opt(c.object_type()),
                        opt_u64(c.version),
                    )),
                    Some(ObjectChangeKind::Published(c)) => published.push(format!(
                        " ┌──\n │ PackageID: {}\n │ Version: {}\n │ Digest: {}\n │ Modules: {}\n └──",
                        opt(c.package_id()),
                        opt_u64(c.version),
                        opt(c.digest()),
                        c.modules.join(", "),
                    )),
                    Some(ObjectChangeKind::Wrapped(c)) => wrapped.push(format!(
                        " ┌──\n │ ObjectID: {}\n │ Sender: {}\n │ ObjectType: {}\n │ Version: {}\n └──",
                        opt(c.object_id()),
                        opt(c.sender()),
                        opt(c.object_type()),
                        opt_u64(c.version),
                    )),
                    Some(ObjectChangeKind::Unwrapped(c)) => unwrapped.push(format!(
                        " ┌──\n │ ObjectID: {}\n │ Sender: {}\n │ Owner: {}\n │ ObjectType: {}\n │ Version: {}\n │ Digest: {}\n └──",
                        opt(c.object_id()),
                        opt(c.sender()),
                        opt(c.owner()),
                        opt(c.object_type()),
                        opt_u64(c.version),
                        opt(c.digest()),
                    )),
                    _ => {}
                }
            }

            write_obj_changes(created, "Created", &mut builder);
            write_obj_changes(deleted, "Deleted", &mut builder);
            write_obj_changes(mutated, "Mutated", &mut builder);
            write_obj_changes(published, "Published", &mut builder);
            write_obj_changes(wrapped, "Wrapped", &mut builder);
            write_obj_changes(unwrapped, "Unwrapped", &mut builder);

            let mut table = builder.build();
            table.with(TablePanel::header("Object Changes"));
            table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
                1,
                TableStyle::modern().get_horizontal(),
            )]));
            writeln!(f, "{table}")?;
        }

        if let Ok(balance_changes) = executed.balance_changes() {
            if !balance_changes.balance_changes.is_empty() {
                let mut builder = TableBuilder::default();
                for balance in &balance_changes.balance_changes {
                    builder.push_record(vec![format!(
                        " ┌──\n │ Owner: {}\n │ CoinType: {}\n │ Amount: {}\n └──",
                        opt(balance.owner()),
                        opt(balance.coin_type()),
                        opt(balance.amount_i128()),
                    )]);
                }
                let mut table = builder.build();
                table.with(TablePanel::header("Balance Changes"));
                table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
                    1,
                    TableStyle::modern().get_horizontal(),
                )]));
                writeln!(f, "{table}")?;
            }
        }

        Ok(())
    }
}

fn write_obj_changes(values: Vec<String>, label: &str, builder: &mut TableBuilder) {
    if !values.is_empty() {
        builder.push_record(vec![format!("{label} Objects: ")]);
        for value in values {
            builder.push_record(vec![value]);
        }
    }
}

fn objref_string(oref: &ObjectReference) -> String {
    format!(
        " ┌──\n │ ObjectID: {}\n │ Version: {}\n │ Digest: {}\n └──",
        oref.object_id(),
        oref.version(),
        oref.digest()
    )
}

/// Render a value parsed from the gRPC result, falling back to `N/A` when the
/// read mask omitted the field or it failed to decode.
fn opt<T: Display, E>(value: Result<T, E>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "N/A".to_string())
}

fn opt_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}
