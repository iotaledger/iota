// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { toast } from 'react-hot-toast';

import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { PasswordModalDialog } from './PasswordInputDialog';

interface UnlockAccountModalProps {
    onClose: () => void;
    onSuccess: () => void;
    account: SerializedUIAccount | null;
    open: boolean;
}

export function UnlockAccountModal({ onClose, onSuccess, account, open }: UnlockAccountModalProps) {
    const backgroundService = useBackgroundClient();
    if (!account) return null;
    return (
        <PasswordModalDialog
            {...{
                open,
                onClose,
                title: 'Unlock wallet',
                confirmText: 'Unlock',
                cancelText: 'Back',
                showForgotPassword: true,
                onSubmit: async (password: string) => {
                    await backgroundService.unlockAccountSourceOrAccount({
                        password,
                        id: account.id,
                    });
                    toast.success('Account unlocked');
                    onSuccess();
                },
                // this is not necessary for unlocking but will show the wrong password error as a form error
                // so doing it like this to keep it simple. The extra verification shouldn't be a problem
                verify: true,
            }}
        />
    );
}
