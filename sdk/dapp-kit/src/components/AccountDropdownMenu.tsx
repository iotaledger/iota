// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';
import type { WalletAccount } from '@iota/wallet-standard';
import { useResolveIotaNSName } from '../hooks/useResolveIotaNSNames.js';
import { useAccounts } from '../hooks/wallet/useAccounts.js';
import { useDisconnectWallet } from '../hooks/wallet/useDisconnectWallet.js';
import { useSwitchAccount } from '../hooks/wallet/useSwitchAccount.js';
import { CheckIcon } from './icons/CheckIcon.js';
import { Text } from './ui/Text.js';
import { Divider, Dropdown, ListItem } from '@iota/apps-ui-kit';
import { Checkmark } from '@iota/ui-icons';
import { useState } from 'react';

type AccountDropdownMenuProps = {
    currentAccount: WalletAccount;
};

export function AccountDropdownMenu({ currentAccount }: AccountDropdownMenuProps) {
    const [isDropdownOpen, setIsDropdownOpen] = useState(false);

    const { mutate: disconnectWallet } = useDisconnectWallet();
    const { mutate: switchAccount } = useSwitchAccount();
    const { data: domain } = useResolveIotaNSName(
        currentAccount.label ? null : currentAccount.address,
    );
    const accounts = useAccounts();

    function handleOnClick(account: WalletAccount) {
        setIsDropdownOpen(!isDropdownOpen);
        switchAccount({ account });
    }

    return !isDropdownOpen ? (
        <ListItem onClick={() => setIsDropdownOpen(!isDropdownOpen)}>
            <div className="flex flex-row gap-xxs">
                <span>
                    {currentAccount.label ?? domain ?? formatAddress(currentAccount.address)}
                </span>
                <Checkmark />
            </div>
        </ListItem>
    ) : (
        <Dropdown>
            <>
                {accounts.map((account) => (
                    <ListItem onClick={() => handleOnClick(account)}>
                        <>
                            <Text mono>
                                {account.label ?? domain ?? formatAddress(account.address)}
                            </Text>
                            {currentAccount.address === account.address ? <CheckIcon /> : null}
                        </>
                    </ListItem>
                ))}
                <Divider />
                <ListItem onClick={() => disconnectWallet()}>
                    <span>Disconnect</span>
                </ListItem>
            </>
        </Dropdown>
    );
}
