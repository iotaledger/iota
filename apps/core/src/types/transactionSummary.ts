// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { GasSummaryType, BalanceChangeSummary, ObjectChangeSummary } from '.';

export type TransactionSummaryType = {
    balanceChanges: BalanceChangeSummary;
    objectSummary: ObjectChangeSummary | null;
    digest?: string;
    sender?: string;
    timestamp?: string | null;
    gas?: GasSummaryType;
} | null;
