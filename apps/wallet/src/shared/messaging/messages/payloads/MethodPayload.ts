// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountSourceSerializedUI } from '_src/background/account-sources/AccountSource';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { type Status } from '_src/background/storage-migration';
import { type SerializedSignature } from '@iota/iota.js/cryptography';

import { isBasePayload } from './BasePayload';
import type { Payload } from './Payload';

export type UIAccessibleEntityType = 'accountSources' | 'accounts';
export type LedgerAccountsPublicKeys = {
    accountID: string;
    publicKey: string;
}[];
export type PasswordRecoveryData =
    | { type: 'mnemonic'; accountSourceID: string; entropy: string }
    | { type: 'seed'; accountSourceID: string; seed: string };

type MethodPayloads = {
    getStoredEntities: { type: UIAccessibleEntityType };
    storedEntitiesResponse: { entities: unknown; type: UIAccessibleEntityType };
    createAccountSource:
        | {
              type: 'mnemonic';
              params: {
                  password: string;
                  entropy?: string;
              };
          }
        | {
              type: 'seed';
              params: {
                  password: string;
                  seed: string;
              };
          };
    accountSourceCreationResponse: { accountSource: AccountSourceSerializedUI };
    lockAccountSourceOrAccount: { id: string };
    unlockAccountSourceOrAccount: { id: string; password?: string };
    createAccounts:
        | { type: 'mnemonic-derived'; sourceID: string }
        | { type: 'seed-derived'; sourceID: string }
        | { type: 'imported'; keyPair: string; password: string }
        | {
              type: 'ledger';
              accounts: { publicKey: string; derivationPath: string; address: string }[];
              password: string;
          };
    accountsCreatedResponse: { accounts: SerializedUIAccount[] };
    signData: { data: string; id: string };
    signDataResponse: { signature: SerializedSignature };
    entitiesUpdated: { type: UIAccessibleEntityType };
    getStorageMigrationStatus: null;
    storageMigrationStatus: { status: Status };
    doStorageMigration: { password: string };
    switchAccount: { accountID: string };
    setAccountNickname: { id: string; nickname: string | null };
    verifyPassword: { password: string };
    storeLedgerAccountsPublicKeys: { publicKeysToStore: LedgerAccountsPublicKeys };
    getAccountSourceEntropy: { accountSourceID: string; password?: string };
    getAccountSourceEntropyResponse: { entropy: string };
    getAccountSourceSeed: { accountSourceID: string; password?: string };
    getAccountSourceSeedResponse: { seed: string };
    clearWallet: {};
    getAutoLockMinutes: {};
    getAutoLockMinutesResponse: { minutes: number | null };
    setAutoLockMinutes: { minutes: number | null };
    notifyUserActive: {};
    getAccountKeyPair: { accountID: string; password: string };
    getAccountKeyPairResponse: { accountID: string; keyPair: string };
    resetPassword: {
        password: string;
        recoveryData: PasswordRecoveryData[];
    };
    verifyPasswordRecoveryData: {
        data: PasswordRecoveryData;
    };
    removeAccount: { accountID: string };
};

type Methods = keyof MethodPayloads;

export interface MethodPayload<M extends Methods> {
    type: 'method-payload';
    method: M;
    args: MethodPayloads[M];
}

export function isMethodPayload<M extends Methods>(
    payload: Payload,
    method: M,
): payload is MethodPayload<M> {
    return (
        isBasePayload(payload) &&
        payload.type === 'method-payload' &&
        'method' in payload &&
        payload.method === method &&
        'args' in payload
    );
}
