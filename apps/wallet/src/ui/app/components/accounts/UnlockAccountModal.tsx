// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';
import { useBackgroundClient } from '_hooks';
import { PasswordModalDialog } from './PasswordInputDialog';
// import type { SerializedUIAccount } from '_src/background/accounts/account';

interface UnlockAccountModalProps {
    onClose: () => void;
    onSuccess: () => void;
    open: boolean;
    // account?: SerializedUIAccount;
}

export function UnlockAccountModal({ onClose, onSuccess, open }: UnlockAccountModalProps) {
    const backgroundService = useBackgroundClient();
    return (
        <PasswordModalDialog
            {...{
                open,
                onClose,
                title: 'Unlock Account',
                confirmText: 'Unlock',
                cancelText: 'Back',
                showForgotPassword: true,
                onSubmit: async (password: string) => {
                    console.log('ONSUBMIT Unlocking all accounts!!', { password });
                    await backgroundService.unlockAllAccounts({
                        password,
                    });
                    toast('Accounts unlocked');
                    onSuccess();
                },
                // this is not necessary for unlocking but will show the wrong password error as a form error
                // so doing it like this to keep it simple. The extra verification shouldn't be a problem
                verify: true,
            }}
        />
    );
}
