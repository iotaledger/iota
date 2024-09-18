// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';
import type { WalletAccount } from '@iota/wallet-standard';
import { useAccounts } from '../hooks/wallet/useAccounts.js';
import { useDisconnectWallet } from '../hooks/wallet/useDisconnectWallet.js';
import { useSwitchAccount } from '../hooks/wallet/useSwitchAccount.js';
import { CheckIcon } from './icons/CheckIcon.js';
import { Divider, ListItem, Button, ButtonType } from '@iota/apps-ui-kit';
import { ArrowDown, ArrowUp } from '@iota/ui-icons';
import { useState } from 'react';

type AccountDropdownMenuProps = {
    currentAccount: WalletAccount;
};

export function AccountDropdownMenu({ currentAccount }: AccountDropdownMenuProps) {
    const [isDropdownOpen, setIsDropdownOpen] = useState(false);

    const { mutate: disconnectWallet } = useDisconnectWallet();
    const { mutate: switchAccount } = useSwitchAccount();

    const accounts = useAccounts();

    function handleOnClick(account: WalletAccount) {
        setIsDropdownOpen(!isDropdownOpen);
        switchAccount({ account });
    }

    return (
        <div className="relative">
            <Button
                onClick={() => setIsDropdownOpen(!isDropdownOpen)}
                type={isDropdownOpen ? ButtonType.Secondary : ButtonType.Outlined}
                text={currentAccount.label ?? formatAddress(currentAccount.address)}
                iconAfterText
                icon={isDropdownOpen ? <ArrowUp /> : <ArrowDown />}
                fullWidth
            />
            {isDropdownOpen && (
                <div className="absolute top-[110%] right-0 w-60 border bg-shader-neutral-light-8 dark:bg-shader-neutral-dark-8 bg-neutral-100 dark:bg-neutral-6 rounded-lg py-xs">
                    {accounts.map((account) => (
                        <ListItem
                            key={account.address}
                            onClick={() => handleOnClick(account)}
                            hideBottomBorder
                        >
                            <>
                                <span className="text-body-lg text-neutra-10 dark:text-neutral-92">
                                    {account.label ?? formatAddress(account.address)}
                                </span>
                                {currentAccount.address === account.address ? <CheckIcon /> : null}
                            </>
                        </ListItem>
                    ))}
                    <Divider />
                    <ListItem onClick={() => disconnectWallet()} hideBottomBorder>
                        <span>Disconnect</span>
                    </ListItem>
                </div>
            )}
        </div>
    );
}
