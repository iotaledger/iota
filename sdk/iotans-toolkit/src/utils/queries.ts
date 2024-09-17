// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient, IotaObjectResponse } from '@iota/iota-sdk/client';

// get NFT's owner from RPC.
export const getOwner = async (client: IotaClient, nftId: string): Promise<string | null> => {
    const ownerResponse = await client.getObject({
        id: nftId,
        options: { showOwner: true },
    });
    const owner = ownerResponse.data?.owner;
    return (
        (owner as { AddressOwner: string })?.AddressOwner ||
        (owner as { ObjectOwner: string })?.ObjectOwner ||
        null
    );
};

// get avatar NFT Object from RPC.
export const getAvatar = async (
    client: IotaClient,
    avatar: string,
): Promise<IotaObjectResponse> => {
    return await client.getObject({
        id: avatar,
        options: {
            showDisplay: true,
            showOwner: true,
        },
    });
};
