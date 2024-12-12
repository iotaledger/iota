// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';
import type { WalletAccount } from '@iota/wallet-standard';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';

import { useAccounts } from '../hooks/wallet/useAccounts.js';
import { useDisconnectWallet } from '../hooks/wallet/useDisconnectWallet.js';
import { useSwitchAccount } from '../hooks/wallet/useSwitchAccount.js';
import * as styles from './AccountDropdownMenu.css.js';
import { CheckIcon } from './icons/CheckIcon.js';
import { ChevronIcon } from './icons/ChevronIcon.js';
import { StyleMarker } from './styling/StyleMarker.js';
import { Button } from './ui/Button.js';
import { Text } from './ui/Text.js';

type AccountDropdownMenuProps = {
    currentAccount: WalletAccount;
    size?: React.ComponentProps<typeof Button>['size'];
};

export function AccountDropdownMenu({ currentAccount, size = 'lg' }: AccountDropdownMenuProps) {
    const { mutate: disconnectWallet } = useDisconnectWallet();
    const accounts = useAccounts();

    return (
        <DropdownMenu.Root modal={false}>
            <StyleMarker>
                <DropdownMenu.Trigger asChild>
                    <Button size={size} className={styles.connectedAccount}>
                        <Text mono weight="bold">
                            {currentAccount.label ?? formatAddress(currentAccount.address)}
                        </Text>
                        <ChevronIcon />
                    </Button>
                </DropdownMenu.Trigger>
            </StyleMarker>
            <DropdownMenu.Portal>
                <StyleMarker className={styles.menuContainer}>
                    <DropdownMenu.Content className={styles.menuContent}>
                        <div className={styles.scrollableContent}>
                            {accounts.map((account) => (
                                <AccountDropdownMenuItem
                                    key={account.address}
                                    account={account}
                                    active={currentAccount.address === account.address}
                                />
                            ))}
                        </div>
                        <DropdownMenu.Separator className={styles.separator} />
                        <DropdownMenu.Item
                            className={clsx(styles.menuItem)}
                            onSelect={() => disconnectWallet()}
                        >
                            Disconnect
                        </DropdownMenu.Item>
                    </DropdownMenu.Content>
                </StyleMarker>
            </DropdownMenu.Portal>
        </DropdownMenu.Root>
    );
}

export function AccountDropdownMenuItem({
    account,
    active,
}: {
    account: WalletAccount;
    active?: boolean;
}) {
    const { mutate: switchAccount } = useSwitchAccount();

    return (
        <DropdownMenu.Item
            className={clsx(styles.menuItem, styles.switchAccountMenuItem)}
            onSelect={() => switchAccount({ account })}
        >
            <Text mono>{account.label ?? formatAddress(account.address)}</Text>
            {active ? <CheckIcon /> : null}
        </DropdownMenu.Item>
    );
}
