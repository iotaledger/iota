// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type { CallArg } from '../../bcs/index.js';

export type IotaJsonValue = boolean | number | string | CallArg | Array<IotaJsonValue>;
export type Order = 'ascending' | 'descending';
export type Unsubscribe = () => Promise<boolean>;
