// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useResolveIotaNSName } from '_app/hooks/useAppResolveIotaNSName';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { Check12, Copy12 } from '@iota/icons';
import { formatAddress } from '@iota/iota-sdk/utils';

import { useCopyToClipboard } from '../hooks/useCopyToClipboard';
import { Text } from '../shared/text';
import { AccountBadge } from './AccountBadge';

export type AccountItemProps = {
    account: SerializedUIAccount;
    onAccountSelected: (account: SerializedUIAccount) => void;
};

/** @deprecated - use AccountListItem from the `accounts` folder **/
export function AccountListItem({ account, onAccountSelected }: AccountItemProps) {
    const { address, type, selected } = account;
    const copy = useCopyToClipboard(address, {
        copySuccessMessage: 'Address Copied',
    });
    const domainName = useResolveIotaNSName(address);

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
                {selected ? <Check12 className="text-success" /> : null}
                <Copy12
                    className="text-gray-60 transition-colors hover:!text-hero-dark group-hover:text-steel"
                    onClick={copy}
                />
            </button>
        </li>
    );
}
