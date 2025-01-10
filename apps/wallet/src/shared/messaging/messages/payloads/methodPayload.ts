// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    AccountSourceType,
    AccountSourceSerializedUI,
} from '_src/background/account-sources/accountSource';
import type { AccountType, SerializedUIAccount } from '_src/background/accounts/account';
import { type Status } from '_src/background/storageMigration';

import { isBasePayload } from './basePayload';
import type { Payload } from './payload';

export type UIAccessibleEntityType = 'accountSources' | 'accounts';
export type LedgerAccountsPublicKeys = {
    accountID: string;
    publicKey: string;
}[];
export type PasswordRecoveryData =
    | { type: AccountSourceType.Mnemonic; accountSourceID: string; entropy: string }
    | { type: AccountSourceType.Seed; accountSourceID: string; seed: string };

type MethodPayloads = {
    getStoredEntities: { type: UIAccessibleEntityType };
    storedEntitiesResponse: { entities: unknown; type: UIAccessibleEntityType };
    createAccountSource:
        | {
              type: AccountSourceType.Mnemonic;
              params: {
                  password: string;
                  entropy?: string;
              };
          }
        | {
              type: AccountSourceType.Seed;
              params: {
                  password: string;
                  seed: string;
              };
          };
    accountSourceCreationResponse: { accountSource: AccountSourceSerializedUI };
    lockAccountSourceOrAccount: { id: string };
    unlockAccountSourceOrAccount: { id: string; password?: string };
    createAccounts:
        | { type: AccountType.MnemonicDerived; sourceID: string }
        | { type: AccountType.SeedDerived; sourceID: string }
        | { type: AccountType.PrivateKeyDerived; keyPair: string; password: string }
        | {
              type: AccountType.LedgerDerived;
              accounts: { publicKey: string; derivationPath: string; address: string }[];
              password: string;
          };
    accountsCreatedResponse: { accounts: SerializedUIAccount[] };
    signData: { data: string; id: string };
    signDataResponse: { signature: string };
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
