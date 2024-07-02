// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PasswordModalDialog, type PasswordModalDialogProps } from './PasswordInputDialog';

type VerifyPasswordModalProps = Pick<PasswordModalDialogProps, 'open' | 'onClose'> & {
    onVerify: (password: string) => Promise<void> | void;
};

export function VerifyPasswordModal({ onClose, onVerify, open }: VerifyPasswordModalProps) {
    return (
        <PasswordModalDialog
            {...{
                onClose,
                open,
                title: 'Verify your wallet password',
                description: 'Enter your wallet password to authenticate',
                verify: true,
                confirmText: 'Verify',
                cancelText: 'Cancel',
                onSubmit: onVerify,
            }}
        />
    );
}
