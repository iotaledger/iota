// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test as setup } from '@playwright/test';

import { toggleSmartContractMode } from './toggleSmartContract';

setup('Authorize Purchases in Smart Contract', async () => {
    await toggleSmartContractMode('purchases');
});
