// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

type ExpirationUnlockCondition = {
    owner: string;
    return_address: string;
    unix_time: number;
};
type StorageDepositReturnUnlockCondition = {
    return_address: string;
    return_amount: string;
};
type TimelockUnlockCondition = {
    unix_time: number;
};

export type CommonOutputObject = {
    id: { id: string };
    balance: string;
    native_tokens: {
        type: string;
        fields: { id: { id: string }; size: string };
    };
};

export interface CommonOutputObjectWithUc extends CommonOutputObject {
    expiration_uc?: {
        type: string;
        fields: ExpirationUnlockCondition;
    };
    storage_deposit_return_uc?: {
        type: string;
        fields: StorageDepositReturnUnlockCondition;
    };
    timelock_uc?: {
        type: string;
        fields: TimelockUnlockCondition;
    };
}

export interface BasicOutputObject extends CommonOutputObjectWithUc {
    metadata?: number[];
    tag?: number[];
    sender?: string;
}

export interface NftOutputObject extends CommonOutputObjectWithUc {}
