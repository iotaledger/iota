// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota.js/client';

const NON_VISUAL_ASSET: IotaObjectData = {
    digest: 'CvsV2zDm2THbQiuAD3PpHC9wizoANGLBZhJsQyaY88Zh',
    objectId: '0x7da24c085e8940230e4853b2f6384be3c775b8f31f97be2f3b75995d7b692297',
    version: '28331430',
    type: '0x3::staking_pool::StakedIota',
};

export const HARDCODED_NON_VISUAL_ASSETS: IotaObjectData[] = new Array(10)
    .fill(0)
    .map(() => NON_VISUAL_ASSET);
