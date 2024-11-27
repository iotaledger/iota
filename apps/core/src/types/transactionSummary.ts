// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { GasSummaryType, BalanceChangeSummary, ObjectChangeSummary } from '.';

export type TransactionSummaryType = {
    digest?: string;
    sender?: string;
    timestamp?: string | null;
    balanceChanges: BalanceChangeSummary;
    gas?: GasSummaryType;
    objectSummary: ObjectChangeSummary | null;
} | null;
