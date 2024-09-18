// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { useCurrentAccount } from '../hooks/wallet/useCurrentAccount.js';
import { AccountDropdownMenu } from './AccountDropdownMenu.js';
import { ConnectModal } from './connect-modal/ConnectModal.js';
import { Button, ButtonType } from '@iota/apps-ui-kit';

type ConnectButtonProps = React.ComponentProps<typeof Button>;

export function ConnectButton({ text = 'Connect', ...buttonProps }: ConnectButtonProps) {
    const currentAccount = useCurrentAccount();
    const [isModalOpen, setModalOpen] = useState(false);

    return currentAccount ? (
        <AccountDropdownMenu currentAccount={currentAccount} />
    ) : (
        <>
            <ConnectModal isModalOpen={isModalOpen} onOpenChange={(open) => setModalOpen(open)} />
            <Button text={text} type={ButtonType.Secondary} {...buttonProps} />
        </>
    );
}
