// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { GasSummaryType, BalanceChangeSummary, ObjectChangesSummary } from '.';

export type TransactionSummaryType = {
    digest?: string;
    sender?: string;
    timestamp?: string | null;
    balanceChanges: BalanceChangeSummary;
    gas?: GasSummaryType;
    objectSummary: ObjectChangesSummary | null;
} | null;
