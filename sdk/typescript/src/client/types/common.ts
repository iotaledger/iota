// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type Order = 'ascending' | 'descending';
export type Unsubscribe = () => Promise<boolean>;
export enum TransactionKind {
    SystemTransaction = 0,
    ProgrammableTransaction = 1,
}
