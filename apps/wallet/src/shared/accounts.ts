// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Bip44Path } from '_src/background/account-sources/bip44Path';

export interface FoundAccount {
    address: string;
    bipPath: Bip44Path;
}
