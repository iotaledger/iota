// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChangeSummary } from './getBalanceChangeSummary';
import { GasSummaryType } from './getGasSummary';
import { ObjectChangeSummary } from './getObjectChangeSummary';

export type TransactionSummary = {
    digest?: string;
    sender?: string;
    timestamp?: string | null;
    balanceChanges: BalanceChangeSummary;
    gas?: GasSummaryType;
    objectSummary: ObjectChangeSummary | null;
} | null;

export type IotaObjectChangeTypes =
    | 'published'
    | 'transferred'
    | 'mutated'
    | 'deleted'
    | 'wrapped'
    | 'created';
