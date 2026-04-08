// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import {
    useBalance,
    useFormatCoin,
    useGetFiatBalance,
    useGetAllBalances,
    NamedAddress,
    toast,
} from '@iota/core';
import { Button, ButtonSize, ButtonType, LoadingIndicator, Panel } from '@iota/apps-ui-kit';
import { getNetwork } from '@iota/iota-sdk/client';
import { ReceiveFundsDialog, SendTokenDialog } from '../dialogs';
import { useCallback, useState } from 'react';
import { trackElementCopied } from '@/lib/utils';

export function AccountBalance() {
    const account = useCurrentAccount();
    const address = account?.address;
    const [isReceiveDialogOpen, setIsReceiveDialogOpen] = useState(false);
    const { network } = useIotaClientContext();
    const { id: networkId, explorer } = getNetwork(network);
    const fiatBalance = useGetFiatBalance(networkId);
    const { data: coinBalance, isPending } = useBalance(address!);
    const [formatted, symbol] = useFormatCoin({ balance: coinBalance?.totalBalance });
    const [isSendTokenDialogOpen, setIsSendTokenDialogOpen] = useState(false);
    const explorerLink = `${explorer}/address/${address}`;
    const { data: coinBalances } = useGetAllBalances(account?.address);

    function openSendTokenDialog(): void {
        setIsSendTokenDialogOpen(true);
    }

    function openReceiveTokenDialog(): void {
        setIsReceiveDialogOpen(true);
    }

    const onCopySuccess = useCallback(() => {
        toast('Address copied');
        trackElementCopied('address');
    }, []);

    const sendTokenCoin = coinBalance?.totalBalance === '0' ? coinBalances?.[0] : coinBalance;

    return (
        <>
            <Panel>
                {isPending ? (
                    <div className="flex h-full w-full items-center justify-center p-2">
                        <LoadingIndicator />
                    </div>
                ) : (
                    <div className="flex h-full flex-col items-center justify-center gap-y-lg p-lg">
                        <div className="flex flex-col items-center gap-y-xs">
                            {address && (
                                <div className="w-full" data-full-address={address} data-amp-mask>
                                    <NamedAddress
                                        address={address}
                                        isCopyable
                                        copyText={address}
                                        isExternal
                                        externalLink={explorerLink}
                                        onCopySuccess={onCopySuccess}
                                        addMarginRightToCenter
                                    />
                                </div>
                            )}
                            <span
                                data-testid="balance-amount"
                                className="text-headline-lg text-iota-neutral-10 dark:text-iota-neutral-92"
                            >
                                {formatted} {symbol}
                            </span>
                            {fiatBalance && (
                                <span className="text-body-md text-iota-neutral-10 dark:text-iota-neutral-92">
                                    {fiatBalance}
                                </span>
                            )}
                        </div>
                        <div className="flex w-full max-w-56 gap-xs">
                            <Button
                                onClick={openSendTokenDialog}
                                text="Send"
                                size={ButtonSize.Small}
                                disabled={!address || coinBalances?.length === 0}
                                testId="send-coin-button"
                                fullWidth
                            />
                            <Button
                                onClick={openReceiveTokenDialog}
                                type={ButtonType.Secondary}
                                text="Receive"
                                size={ButtonSize.Small}
                                fullWidth
                            />
                        </div>
                    </div>
                )}
                {address && (
                    <>
                        {sendTokenCoin && (
                            <SendTokenDialog
                                activeAddress={address}
                                coin={sendTokenCoin}
                                open={isSendTokenDialogOpen}
                                setOpen={setIsSendTokenDialogOpen}
                            />
                        )}
                        <ReceiveFundsDialog
                            address={address}
                            open={isReceiveDialogOpen}
                            setOpen={setIsReceiveDialogOpen}
                        />
                    </>
                )}
            </Panel>
        </>
    );
}
