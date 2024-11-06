// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClientContext, useIotaClientQuery } from '@iota/dapp-kit';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    COINS_QUERY_REFETCH_INTERVAL,
    COINS_QUERY_STALE_TIME,
    filterAndSortTokenBalances,
    useBalance,
    useFormatCoin,
} from '@iota/core';
import { Address, Button, ButtonSize, ButtonType, Panel } from '@iota/apps-ui-kit';
import { CoinBalance, getNetwork } from '@iota/iota-sdk/client';
import { SendCoinPopup } from '../Popup';
import { ReceiveFundsDialog } from '../Dialogs';
import { usePopups } from '@/hooks';
import toast from 'react-hot-toast';
import { useState } from 'react';

export function AccountBalance() {
    const account = useCurrentAccount();
    const address = account?.address;
    const { openPopup, closePopup } = usePopups();
    const [isReceiveDialogOpen, setIsReceiveDialogOpen] = useState(false);
    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);
    const { data: coinBalance, isPending } = useBalance(address!);
    const formattedAddress = formatAddress(address!);
    const [formatted, symbol] = useFormatCoin(coinBalance?.totalBalance, IOTA_TYPE_ARG);
    const { data: coinBalances } = useIotaClientQuery(
        'getAllBalances',
        { owner: address! },
        {
            enabled: !!address,
            staleTime: COINS_QUERY_STALE_TIME,
            refetchInterval: COINS_QUERY_REFETCH_INTERVAL,
            select: filterAndSortTokenBalances,
        },
    );
    const explorerLink = `${explorer}/address/${address}`;

    function openSendTokenPopup(coin: CoinBalance, address: string): void {
        if (coinBalances) {
            openPopup(
                <SendCoinPopup
                    coin={coin}
                    senderAddress={address}
                    onClose={closePopup}
                    coins={coinBalances}
                />,
            );
        }
    }

    function openReceiveTokenPopup(): void {
        setIsReceiveDialogOpen(true);
    }

    function handleOnCopySuccess() {
        toast.success('Address copied');
    }

    return (
        <>
            <Panel>
                {isPending && <p>Loading...</p>}
                {!isPending && (
                    <div className="flex h-full flex-col items-center justify-center gap-y-lg p-lg">
                        <div className="flex h-full flex-col items-center justify-center gap-y-xs">
                            {address && (
                                <Address
                                    text={formattedAddress}
                                    isCopyable
                                    copyText={address}
                                    isExternal
                                    externalLink={explorerLink}
                                    onCopySuccess={handleOnCopySuccess}
                                />
                            )}
                            <span className="text-headline-lg text-neutral-10 dark:text-neutral-92">
                                {formatted} {symbol}
                            </span>
                        </div>
                        <div className="flex w-full max-w-56 gap-xs">
                            <Button
                                onClick={() =>
                                    coinBalance &&
                                    openSendTokenPopup(coinBalance, account?.address ?? '')
                                }
                                text="Send"
                                size={ButtonSize.Small}
                                disabled={!address}
                                testId="send-coin-button"
                                fullWidth
                            />
                            <Button
                                onClick={openReceiveTokenPopup}
                                type={ButtonType.Secondary}
                                text="Receive"
                                size={ButtonSize.Small}
                                fullWidth
                            />
                        </div>
                    </div>
                )}
            </Panel>
            <ReceiveFundsDialog
                address={address!}
                open={isReceiveDialogOpen}
                setOpen={setIsReceiveDialogOpen}
            />
        </>
    );
}
