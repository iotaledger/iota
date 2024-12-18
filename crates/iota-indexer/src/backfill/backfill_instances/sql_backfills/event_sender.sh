# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

INDEXER=${INDEXER:-"iota-indexer"}
DB=${DB:-"postgres://postgres:postgrespw@localhost:5432/postgres"}
"$INDEXER" --database-url "$DB" run-back-fill "$1" "$2" sql "UPDATE events SET sender = CASE WHEN cardinality(senders) > 0 THEN senders[1] ELSE NULL END" checkpoint_sequence_number
