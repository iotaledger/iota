// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectResponse } from '@iota/iota.js/client';

export const hasDisplayData = (obj: IotaObjectResponse) => !!obj.data?.display?.data;
