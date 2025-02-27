// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type Order = 'ascending' | 'descending';
export type Unsubscribe = () => Promise<boolean>;
export enum TransactionKind {
    ProgrammableTransaction = 'ProgrammableTransaction',
    Genesis = 'Genesis',
    ConsensusCommitPrologueV1 = 'ConsensusCommitPrologueV1',
    AuthenticatorStateUpdateV1 = 'AuthenticatorStateUpdateV1',
    RandomnessStateUpdate = 'RandomnessStateUpdate',
    EndOfEpochTransaction = 'EndOfEpochTransaction',
    SystemTransaction = 'SystemTransaction',
}
