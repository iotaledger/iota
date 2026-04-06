// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType } from '_src/background/accounts/account';

export const NEW_TAB_ACCOUNT_TYPES = [AccountType.PasskeyDerived];

export const ACCOUNT_TYPES_WITH_SOURCE: AccountType[] = [
    AccountType.MnemonicDerived,
    AccountType.SeedDerived,
    AccountType.KeystoneDerived,
];
