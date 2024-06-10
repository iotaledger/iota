// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetObject } from '@iota/core';
import { type IotaObjectData } from '@iota/iota.js/client';

export function useOwnedNFT(nftObjectId: string | null) {
    const objectResult = useGetObject(nftObjectId);

    return {
        ...objectResult,
        data: objectResult.data?.data as IotaObjectData | null,
    };
}
