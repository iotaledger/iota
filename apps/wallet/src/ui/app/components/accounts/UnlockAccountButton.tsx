// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';

import { Button } from '../../shared/ButtonUI';
import { useUnlockAccount } from './UnlockAccountContext';

export type UnlockAccountButtonProps = {
    account: SerializedUIAccount;
    title?: string;
};
export function UnlockAccountButton({
    account,
    title = 'Unlock Account',
}: UnlockAccountButtonProps) {
    const { isPasswordUnlockable } = account;
    const { unlockAccount } = useUnlockAccount();

    if (isPasswordUnlockable) {
        return <Button text={title} onClick={() => unlockAccount(account)} />;
    }
}
