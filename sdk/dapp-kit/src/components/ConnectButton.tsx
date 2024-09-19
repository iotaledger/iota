// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { useCurrentAccount } from '../hooks/wallet/useCurrentAccount.js';
import { AccountDropdownMenu } from './AccountDropdownMenu.js';
import { ConnectModal } from './connect-modal/ConnectModal.js';
import { Button, ButtonType } from '@iota/apps-ui-kit';

export function ConnectButton(props: ConnectButtonWithModalProps) {
    const currentAccount = useCurrentAccount();

    return currentAccount ? (
        <AccountDropdownMenu currentAccount={currentAccount} />
    ) : (
        <ConnectButtonWithModal {...props} />
    );
}

type ConnectButtonWithModalProps = React.ComponentProps<typeof Button>;
function ConnectButtonWithModal({ text = 'Connect', ...buttonProps }: ConnectButtonWithModalProps) {
    const [isModalOpen, setModalOpen] = useState(false);
    return (
        <>
            <ConnectModal isModalOpen={isModalOpen} onOpenChange={(open) => setModalOpen(open)} />
            <Button text={text} type={ButtonType.Secondary} {...buttonProps} />
        </>
    );
}
