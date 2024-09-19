// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '../hooks/wallet/useCurrentAccount.js';
import { AccountDropdownMenu } from './AccountDropdownMenu.js';
import { ConnectModal } from './connect-modal/ConnectModal.js';
import { Button, ButtonType } from '@iota/apps-ui-kit';

type ConnectButtonProps = React.ComponentProps<typeof Button>;
export function ConnectButton({ text = 'Connect', ...buttonProps }: ConnectButtonProps) {
    const currentAccount = useCurrentAccount();

    return currentAccount ? (
        <AccountDropdownMenu currentAccount={currentAccount} />
    ) : (
        <ConnectModal
            trigger={<Button text={text} type={ButtonType.Secondary} {...buttonProps} />}
        />
    );
}
