// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange } from '../interfaces';

export type BalanceChangeByOwner = Record<string, BalanceChange[]>;
export type BalanceChangeSummary = BalanceChangeByOwner | null;
