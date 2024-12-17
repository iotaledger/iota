// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';
import { CommonMigrationObjectType } from '../enums';

export type UnlockConditionTimestamp = string;

interface CommonExpirationTypeObject {
    unlockConditionTimestamp: UnlockConditionTimestamp;
    output: IotaObjectData;
    uniqueId: string;
    balance: bigint;
}

export interface ResolvedNativeToken extends CommonExpirationTypeObject {
    name: string;
    commonObjectType: CommonMigrationObjectType.NativeToken;
    coinType: string;
}

export interface ResolvedBasicObject extends CommonExpirationTypeObject {
    type: string;
    commonObjectType: CommonMigrationObjectType.Basic;
}

export interface ResolvedNftObject extends CommonExpirationTypeObject {
    name: string;
    image_url: string;
    commonObjectType: CommonMigrationObjectType.Nft;
}

export type ResolvedObjectTypes = ResolvedBasicObject | ResolvedNftObject | ResolvedNativeToken;
