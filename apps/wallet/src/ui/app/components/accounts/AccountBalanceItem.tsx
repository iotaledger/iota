// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Copy12 } from '@iota/icons';
import { Text } from '_src/ui/app/shared/text';
import { formatAddress } from '@iota/iota.js/utils';
import { useCopyToClipboard } from '../../hooks/useCopyToClipboard';
import { IconButton } from '../IconButton';
import { type AddressFromFinder } from '_src/shared/accounts';

interface AccountBalanceItemProps {
    finderAddress: AddressFromFinder;
}

export function AccountBalanceItem({
    finderAddress: finderAccount,
}: AccountBalanceItemProps): JSX.Element {
    const copyAddress = useCopyToClipboard(finderAccount.pubKeyHash, {
        copySuccessMessage: 'Address copied',
    });
    return (
        <div className="group flex cursor-pointer flex-col gap-3 rounded-xl border border-solid border-hero/10 bg-white/40 px-4 py-3">
            <div className="flex w-full flex-row items-center justify-between">
                <div className="flex gap-1.5 leading-none text-steel-dark">
                    <Text variant="body" weight="semibold">
                        {formatAddress(finderAccount.pubKeyHash)}
                    </Text>
                    <div className="flex gap-1 opacity-0 duration-100 group-hover:opacity-100">
                        <IconButton
                            variant="transparent"
                            icon={<Copy12 className="h-2.5 w-2.5" />}
                            onClick={copyAddress}
                        />
                    </div>
                </div>

                <Text variant="bodySmall" weight="semibold" color="steel-darker">
                    {finderAccount.balance.totalBalance} IOTA
                </Text>
            </div>
        </div>
    );
}
