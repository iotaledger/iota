// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PasswordModalDialog, type PasswordModalDialogProps } from './PasswordInputDialog';

interface VerifyPasswordModalProps {
    open: PasswordModalDialogProps['open'];
    onClose: PasswordModalDialogProps['onClose'];
    onVerify: (password: string) => Promise<void> | void;
}

export function VerifyPasswordModal({ onClose, onVerify, open }: VerifyPasswordModalProps) {
    return (
        <PasswordModalDialog
            {...{
                onClose,
                open,
                title: 'Verify your wallet password',
                verify: true,
                confirmText: 'Verify',
                cancelText: 'Back',
                onSubmit: onVerify,
            }}
        />
    );
}
