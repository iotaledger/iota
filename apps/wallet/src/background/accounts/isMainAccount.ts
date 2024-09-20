// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isLedgerAccountSerializedUI } from '_src/background/accounts/LedgerAccount';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/MnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/SeedAccount';
import { parseDerivationPath } from '_src/background/account-sources/bip44Path';
import type { SerializedUIAccount } from '_src/background/accounts/Account';

export function isMainAccount(account: SerializedUIAccount | null) {
    {
        if (!account) {
            return false;
        }

        if (
            isLedgerAccountSerializedUI(account) ||
            isMnemonicSerializedUiAccount(account) ||
            isSeedSerializedUiAccount(account)
        ) {
            const { addressIndex, changeIndex, accountIndex } = parseDerivationPath(
                account.derivationPath,
            );

            return addressIndex === 0 && changeIndex === 0 && accountIndex === 0;
        }
    }
}
