// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';
import { CommonMigrationObjectType } from '../enums';

export type ExpirationMigrationKey = string;
interface CommonExpirationTypeObject {
    expirationKey: ExpirationMigrationKey;
    output: IotaObjectData;
    uniqueId: string;
    balance: number;
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

export interface ResolvedObjectsGrouped {
    nftObjects: Record<ExpirationMigrationKey, NftObjectsResolvedList>;
    basicObjects: Record<ExpirationMigrationKey, BasicObjectsResolvedList>;
    nativeTokens: Record<ExpirationMigrationKey, NativeTokensResolvedList>;
}

export type NftObjectsResolvedList = ResolvedNftObject[];
export type BasicObjectsResolvedList = ResolvedBasicObject;
export type NativeTokensResolvedList = Record<string, ResolvedNativeToken>;

export type ResolvedObjectTypes = ResolvedBasicObject | ResolvedNftObject | ResolvedNativeToken;

export type ExpirationObjectListEntries = [string, ResolvedObjectsList][];

export type ResolvedObjectsList =
    | NftObjectsResolvedList
    | BasicObjectsResolvedList
    | NativeTokensResolvedList;
