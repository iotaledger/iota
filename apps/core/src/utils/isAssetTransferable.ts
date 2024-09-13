// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';

export const isAssetTransferable = (obj: IotaObjectData | null | undefined): boolean =>
    !!obj && obj.content?.dataType === 'moveObject' && obj.content.hasPublicTransfer;
