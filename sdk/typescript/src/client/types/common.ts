// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import type { IotaSystemStateSummaryV1, IotaSystemStateSummary } from './index.js';
export type Order = 'ascending' | 'descending';
export type Unsubscribe = () => Promise<boolean>;
export type IotaSystemStateSummaryCompat = IotaSystemStateSummaryV1 | IotaSystemStateSummary;
