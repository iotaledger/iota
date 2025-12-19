// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import mitt from 'mitt';
import type { EventEmissionOptions } from '../account-sources/events';

type AccountsEvents = {
    accountsChanged: void;
    accountStatusChanged: { accountID: string };
    activeAccountChanged: { accountID: string };
};

export const accountsEvents = mitt<AccountsEvents>();

export type LockAccountOptions = {
    allowRead?: boolean;
} & EventEmissionOptions;

export type UnlockAccountOptions = EventEmissionOptions;
