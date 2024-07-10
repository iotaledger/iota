// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { useResolveIotaNSName } from '@iota/core';
// import { Checkmark, Copy } from '@iota/icons';
import { formatAddress } from '@iota/iota.js/utils';
import { Text } from '../shared/text';
import { AccountBadge } from './AccountBadge';

export interface AccountItemProps {
    account: SerializedUIAccount;
    onAccountSelected: (account: SerializedUIAccount) => void;
}

/** @deprecated - use AccountListItem from the `accounts` folder **/
export function AccountListItem({ account, onAccountSelected }: AccountItemProps) {
    const { address, type } = account;
    // const copy = useCopyToClipboard(address, {
    //     copySuccessMessage: 'Address Copied',
    // });
    const { data: domainName } = useResolveIotaNSName(address);

    return (
        <li>
            <button
                className="group flex w-full cursor-pointer appearance-none items-center gap-2.5 rounded-md border-0 bg-transparent p-2.5 text-left transition-colors hover:bg-iota/10 focus-visible:ring-1"
                onClick={() => {
                    onAccountSelected(account);
                }}
            >
                <div className="flex min-w-0 flex-1 items-center gap-2">
                    <div className="min-w-0">
                        <Text color="steel-darker" variant="bodySmall" truncate mono>
                            {domainName ?? formatAddress(address)}
                        </Text>
                    </div>
                    <AccountBadge accountType={type} />
                </div>
                {/* {selected ? <Checkmark className="text-success" /> : null}
                <Copy
                    className="text-gray-60 transition-colors hover:!text-hero-dark group-hover:text-steel"
                    onClick={copy}
                /> */}
            </button>
        </li>
    );
}
