// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Warning } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonPill,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    DialogPosition,
    DisplayStats,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Input,
    InputType,
    LoadingIndicator,
    Panel,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { normalizeIotaName } from '@iota/iota-names-sdk';
import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_DECIMALS, safeParseAmount } from '@iota/iota-sdk/utils';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';

import { useAuctionBid } from '@/auctions/hooks/useAuctionBid';
import { useCountdown } from '@/auctions/hooks/useCountdown';
import { useGetAuctionMetadata } from '@/auctions/hooks/useGetAuctionMetadata';
import { formatTimeRemaining, getTimeRemaining, getUserAuctionStatus } from '@/auctions/lib/utils';
import { NameRecordData, queryKey, useCalculatePriceInFiat, useNameRecord } from '@/hooks';
import {
    formatNanosToIota,
    getUserFriendlyErrorMessage,
    parseIotaToNanos,
    parseNanosToIota,
} from '@/lib/utils';
import { ampli } from '@/lib/utils/analytics/ampli';
import { formatExpirationDate } from '@/lib/utils/format/formatExpirationDate';

interface AuctionBidDialogDialogProps {
    name: string;
    closeDialog: () => void;
    onCompleted?: () => void;
}

export function AuctionBidDialog({ name, closeDialog, onCompleted }: AuctionBidDialogDialogProps) {
    const iotaClient = useIotaClient();
    const account = useCurrentAccount();
    const queryClient = useQueryClient();
    const { data: nameRecordData, isLoading: isNameRecordLoading } = useNameRecord(name);

    const nameRecord = nameRecordData as Extract<NameRecordData, { type: 'available' }> | undefined;
    const { data: auctionMetadata } = useGetAuctionMetadata(name);
    const minBidNanos =
        auctionMetadata?.minBidNanos || (nameRecord ? BigInt(nameRecord.price) : null);
    const auctionStatus = getUserAuctionStatus(auctionMetadata ?? null, account?.address || '');

    const [bidAmountValue, setBidAmountValue] = useState<string | undefined>();

    const currentPrice =
        bidAmountValue ||
        (minBidNanos
            ? formatNanosToIota(minBidNanos, { formatRounded: false, showIotaSymbol: false })
            : '');

    const currentPriceInNanos = parseIotaToNanos(currentPrice) || BigInt(0);
    const fiatPrice = useCalculatePriceInFiat(currentPriceInNanos);

    // Sync the minimum bid amount
    useEffect(() => {
        if (minBidNanos) {
            setBidAmountValue(parseNanosToIota(minBidNanos).toString());
        }
    }, [minBidNanos]);

    const bidNanos = bidAmountValue ? safeParseAmount(bidAmountValue, IOTA_DECIMALS) : null;

    const {
        data: auctionBidTransaction,
        isLoading: isAuctionBidLoading,
        isPending: isAuctionBidPending,
        error: auctionError,
    } = useAuctionBid({
        name,
        bidNanos: bidNanos ?? BigInt(0),
    });

    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();

    const { mutateAsync: handleConfirm, isPending: isSigningTransaction } = useMutation({
        async mutationFn(auctionBidTransaction: Transaction) {
            const transactionResult = await signAndExecuteTransaction({
                transaction: auctionBidTransaction,
            });

            const response = await iotaClient.waitForTransaction({
                digest: transactionResult.digest,
                options: {
                    showEffects: true,
                },
            });

            if (response?.effects?.status.status === 'failure') {
                throw new Error('Transaction failed');
            }
        },
        onSuccess() {
            queryClient.invalidateQueries({
                queryKey: queryKey.userAuctionHistory(account?.address),
            });
            queryClient.invalidateQueries({ queryKey: queryKey.auctionList() });
            queryClient.invalidateQueries({ queryKey: queryKey.auctionMetadata(name) });

            if (!auctionMetadata) {
                ampli.placedAuctionBid({
                    name: name,
                    wasUserTopBidder: false,
                    isUserFirstBidOnAuction: true,
                    auctionCurrentBidAmount: 0,
                    userBidAmount: parseNanosToIota(bidNanos ?? 0),
                });
            } else if (auctionStatus === 'top_bidder') {
                ampli.placedAuctionBid({
                    name: name,
                    wasUserTopBidder: true,
                    isUserFirstBidOnAuction: false,
                    auctionCurrentBidAmount: parseNanosToIota(auctionMetadata.currentBidNanos),
                    userBidAmount: parseNanosToIota(bidNanos ?? 0),
                });
            } else {
                ampli.placedAuctionBid({
                    name: name,
                    wasUserTopBidder: false,
                    isUserFirstBidOnAuction: false,
                    auctionCurrentBidAmount: parseNanosToIota(auctionMetadata.currentBidNanos),
                    userBidAmount: parseNanosToIota(bidNanos ?? 0),
                });
            }

            toast.success(
                `Successfully placed bid of ${formatNanosToIota(bidNanos ?? 0, {
                    formatRounded: false,
                    showIotaSymbol: true,
                })} on ${normalizeIotaName(name)}`,
            );
            closeDialog();
            onCompleted?.();
        },
        onError(err) {
            toast.error(getUserFriendlyErrorMessage(err));
        },
    });

    const status = auctionMetadata && getUserAuctionStatus(auctionMetadata, account?.address || '');
    const timeRemainingMs = auctionMetadata && getTimeRemaining(auctionMetadata);
    const { milliseconds } = useCountdown(timeRemainingMs || 0);

    const isBidBelowMinimum = minBidNanos ? (bidNanos || BigInt(0)) < minBidNanos : false;

    const isLoading =
        isNameRecordLoading || isAuctionBidLoading || isSendingTransaction || isSigningTransaction;
    const isPending = isAuctionBidPending;
    const disablePlaceBid = isPending || isLoading || isBidBelowMinimum || !auctionBidTransaction;

    const formattedTimeRemaining = formatTimeRemaining(milliseconds);
    const currentBid = auctionMetadata
        ? formatNanosToIota(auctionMetadata.currentBidNanos, {
              formatRounded: false,
              showIotaSymbol: true,
          })
        : '--';
    const expirationDate = auctionMetadata
        ? formatExpirationDate(auctionMetadata.nftExpiration)
        : '--';
    const minBidLabel = minBidNanos
        ? formatNanosToIota(minBidNanos, {
              formatRounded: false,
              showIotaSymbol: true,
          })
        : '--';
    const minBidWithoutLabel = minBidNanos
        ? formatNanosToIota(minBidNanos, {
              formatRounded: false,
              showIotaSymbol: false,
          })
        : '--';

    const errorMessage = isBidBelowMinimum ? `Bid must be ≥ ${minBidLabel}` : undefined;

    return (
        <Dialog open onOpenChange={closeDialog}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header title="Auction" titleCentered onClose={() => closeDialog()} />

                <DialogBody>
                    <div className="flex flex-col justify-between h-full items-center">
                        <div className="flex flex-col w-full gap-y-md">
                            {status === 'top_bidder' && (
                                <InfoBox
                                    title="Top Bidder"
                                    supportingText="Your are the top bidder already"
                                    icon={<Warning />}
                                    type={InfoBoxType.Warning}
                                    style={InfoBoxStyle.Default}
                                />
                            )}

                            <Panel data-amp-mask bgColor="bg-names-neutral-12">
                                <div className="px-md py-lg">
                                    <span className="text-names-neutral-100 text-headline-sm break-words overflow-hidden">
                                        {normalizeIotaName(name)}
                                    </span>
                                </div>
                            </Panel>
                            {auctionMetadata && (
                                <div className="flex flex-row gap-x-sm w-full">
                                    <DisplayStats
                                        label="Current Bid"
                                        value={currentBid}
                                        tooltipText="The current highest bid for this auction."
                                        tooltipPosition={TooltipPosition.Right}
                                    />
                                    <DisplayStats
                                        label="Time Left"
                                        value={formattedTimeRemaining}
                                    />
                                </div>
                            )}
                            <Input
                                type={InputType.NumericFormat}
                                label="Your Bid"
                                min={Number(minBidNanos)}
                                value={bidAmountValue}
                                supportingText={fiatPrice ? `$${fiatPrice}` : ''}
                                onChange={({ target: { value } }) => setBidAmountValue(value)}
                                errorMessage={errorMessage}
                                trailingElement={
                                    <ButtonPill
                                        onClick={() => setBidAmountValue(minBidWithoutLabel)}
                                    >
                                        Min
                                    </ButtonPill>
                                }
                            />
                        </div>
                        <div className="flex w-full flex-col gap-y-md">
                            <hr className="w-full border-0 border-t border-names-neutral-20" />
                            <div className="flex flex-col gap-y-xs text-body-md text-names-neutral-50 px-4 pb-4">
                                <p>
                                    If someone <strong>outbids</strong> you, the amount you placed
                                    is fully refunded.
                                </p>
                                <p>
                                    <strong>Auctions run</strong> for at{' '}
                                    <strong>least 48 hours</strong> and extend if a bid is placed in
                                    the final 10 minutes.
                                </p>
                                <p>
                                    <strong>Win the auction?</strong> Don't forget to claim your
                                    name. The 1-year subscription starts at the auction's start
                                    time.
                                </p>
                            </div>
                            {auctionMetadata && (
                                <DisplayStats label="Registration Expires" value={expirationDate} />
                            )}
                            {auctionError ? (
                                <InfoBox
                                    type={InfoBoxType.Error}
                                    style={InfoBoxStyle.Elevated}
                                    icon={<Warning />}
                                    title="Error"
                                    supportingText={getUserFriendlyErrorMessage(auctionError)}
                                />
                            ) : null}
                            <div className="flex w-full flex-row gap-x-xs">
                                <Button
                                    type={ButtonType.Secondary}
                                    text="Cancel"
                                    onClick={() => closeDialog()}
                                    fullWidth
                                />
                                <Button
                                    type={ButtonType.Primary}
                                    disabled={disablePlaceBid}
                                    icon={isLoading || isPending ? <LoadingIndicator /> : null}
                                    text={auctionMetadata ? 'Bid' : 'Start auction'}
                                    onClick={() => {
                                        if (auctionBidTransaction) {
                                            handleConfirm(auctionBidTransaction.transaction);
                                        }
                                    }}
                                    fullWidth
                                />
                            </div>
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
