// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';

export type AssetsDialogView =
    | {
          type: 'details';
          asset: IotaObjectData;
      }
    | {
          type: 'send';
          asset: IotaObjectData;
      }
    | {
          type: 'close';
          asset: undefined;
      };
