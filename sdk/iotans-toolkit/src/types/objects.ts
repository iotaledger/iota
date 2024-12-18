// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type IotaNSContract = {
    packageId: string;
    iotans: string;
    registry: string;
    reverseRegistry: string;
};

export type NameObject = {
    id: string;
    owner: string;
    targetAddress: string;
    avatar?: string;
    contentHash?: string;
};

export type DataFields = 'avatar' | 'contentHash';

export type NetworkType = 'devnet' | 'testnet';
