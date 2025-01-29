// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';
import { parseDerivationPath } from '_src/background/account-sources/bip44Path';
import type { SerializedUIAccount } from '_src/background/accounts/account';

export function isLegacyAccount(account: SerializedUIAccount | null) {
    if (!account) {
        return false;
    }

    if (isMnemonicSerializedUiAccount(account) || isSeedSerializedUiAccount(account)) {
        const { addressIndex, changeIndex } = parseDerivationPath(account.derivationPath);

        return addressIndex !== 0 || changeIndex !== 0;
    } else {
        return false;
    }
}
