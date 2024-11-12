// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';

export const isAssetTransferable = (obj: IotaObjectData | null | undefined): boolean =>
    // TODO: Either the type abilities will be added to 'IotaParsedData' and
    // we need to check if the object has the 'store' ability or there will be a new endpoint
    // that returns the "transferable" status of a MoveType.
    !!obj && obj.content?.dataType === 'moveObject'; // && obj.content.hasPublicTransfer;
