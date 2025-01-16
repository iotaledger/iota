// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectData } from '@iota/iota-sdk/client';
import { useMediaUrl } from './useMediaUrl';
import { useFileExtensionType } from './useFileExtensionType';

export function useNFTBasicData(nftObj: IotaObjectData | null) {
    const nftObjectID = nftObj?.objectId || null;
    const filePath = useMediaUrl(nftObj?.content || null);
    let objType = null;
    let nftFields = null;
    if (nftObj && nftObj.content?.dataType === 'moveObject') {
        objType = nftObj.content?.type;
        nftFields = nftObj?.content?.dataType === 'moveObject' ? nftObj.content.fields : null;
    }
    const fileExtensionType = useFileExtensionType(filePath || '');
    return {
        nftObjectID,
        filePath,
        nftFields,
        fileExtensionType,
        objType,
    };
}
