// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectResponse } from '@iota/iota-sdk/client';

export const hasDisplayData = (obj: IotaObjectResponse) => !!obj.data?.display?.data;
