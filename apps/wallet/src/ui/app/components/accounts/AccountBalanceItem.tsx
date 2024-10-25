// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';
import { useCopyToClipboard } from '../../hooks/useCopyToClipboard';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { useBalance, useFormatCoin } from '@iota/core';
import { Copy } from '@iota/ui-icons';
import { ButtonUnstyled } from '@iota/apps-ui-kit';

interface AccountBalanceItemProps {
    account: SerializedUIAccount;
}

export function AccountBalanceItem({ account }: AccountBalanceItemProps): JSX.Element {
    const copyAddress = useCopyToClipboard(account.address, {
        copySuccessMessage: 'Address copied',
    });
    const { data: balance } = useBalance(account.address, {
        refetchInterval: false,
    });

    const totalBalance = balance?.totalBalance || '0';
    const coinType = balance?.coinType;
    const [formatted, symbol] = useFormatCoin(BigInt(totalBalance), coinType);

    return (
        <div className="border-hero/10 group flex cursor-pointer flex-col gap-3 rounded-xl border border-solid bg-white/40 px-4 py-3">
            <div className="flex w-full flex-row items-center justify-between">
                <div className="text-steel-dark flex gap-1.5 leading-none">
                    <span className="text-body-md">{formatAddress(account.address)}</span>
                    <div className="flex gap-1 opacity-0 duration-100 group-hover:opacity-100">
                        <ButtonUnstyled onClick={copyAddress}>
                            <Copy className="h-2.5 w-2.5" />
                        </ButtonUnstyled>
                    </div>
                </div>

                <span className="text-body-sm">
                    {formatted} {symbol}
                </span>
            </div>
        </div>
    );
}
