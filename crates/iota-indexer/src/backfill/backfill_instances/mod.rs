// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use crate::backfill::{BackfillTaskKind, backfill_task::BackfillTask};

mod sql_backfill;

pub(crate) fn get_backfill_task(kind: BackfillTaskKind) -> Arc<dyn BackfillTask> {
    match kind {
        BackfillTaskKind::Sql { sql, key_column } => {
            Arc::new(sql_backfill::SqlBackFill::new(sql, key_column))
        }
    }
}
