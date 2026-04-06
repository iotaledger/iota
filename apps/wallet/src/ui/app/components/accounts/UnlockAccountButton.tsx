// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { useUnlockAccounts } from './UnlockAccountsContext';
import { Button, ButtonType } from '@iota/apps-ui-kit';

export interface UnlockAccountButtonProps {
    account: SerializedUIAccount;
    title?: string;
}

export function UnlockAccountButton({
    account,
    title = 'Unlock your Account',
}: UnlockAccountButtonProps) {
    const { isPasswordUnlockable } = account;
    const { unlockAccounts } = useUnlockAccounts();

    if (isPasswordUnlockable) {
        return (
            <Button
                text={title}
                onClick={() => unlockAccounts()}
                type={ButtonType.Secondary}
                fullWidth
            />
        );
    }
}
