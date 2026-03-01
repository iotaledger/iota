// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';
import { useUnlockMutation } from '_hooks';
import { PasswordModalDialog } from './PasswordModalDialog';
import { useState } from 'react';
import { ForgotPasswordDialog } from './ForgotPasswordDialog';

interface UnlockAccountModalProps {
    onClose: () => void;
    onSuccess: () => void;
    open: boolean;
}

export function UnlockAccountModal({ onClose, onSuccess, open }: UnlockAccountModalProps) {
    const [isForgotPasswordOpen, setForgotPasswordOpen] = useState(false);
    const unlockAllAccountsMutation = useUnlockMutation();

    return (
        <>
            <PasswordModalDialog
                open={open && !isForgotPasswordOpen}
                onClose={onClose}
                title="Unlock wallet"
                confirmText="Unlock wallet"
                cancelText="Back"
                showForgotPassword={true}
                onForgotPassword={() => {
                    setForgotPasswordOpen(true);
                }}
                onSubmit={async (password: string) => {
                    await unlockAllAccountsMutation.mutateAsync({
                        password,
                    });
                    toast('Wallet unlocked');
                    onSuccess();
                }}
                // this is not necessary for unlocking but will show the wrong password error as a form error
                // so doing it like this to keep it simple. The extra verification shouldn't be a problem
                verify={true}
                isLoading={unlockAllAccountsMutation.isPending}
            />
            <ForgotPasswordDialog isOpen={isForgotPasswordOpen} setOpen={setForgotPasswordOpen} />
        </>
    );
}
