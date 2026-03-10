// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Clock, IotaLogoSmall, Loader } from '@iota/apps-ui-icons';
import { Button, ButtonType, Card, CardType } from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import { normalizeIotaName } from '@iota/iota-names-sdk';
import { MouseEvent, useMemo } from 'react';

import {
    AuctionDetails,
    AuctionStatusBadge,
    formatTimeRemaining,
    getTimeRemaining,
    getUserAuctionStatus,
} from '@/auctions';
import { useCountdown } from '@/auctions/hooks/useCountdown';
import { NameCard } from '@/components/name-card/NameCard';
import { NameCardBody } from '@/components/name-card/NameCardBody';
import { useCalculatePriceInFiat, useNameRecord } from '@/hooks';
import { FORBIDDEN_LIST } from '@/lib/constants/forbiddenList';
import { formatNanosToIota } from '@/lib/utils';
import { censorName } from '@/lib/utils/censorName';
import { getNameDisplaySrc } from '@/lib/utils/displayImage';

import { useAuctionRefresh } from '../hooks/useAuctionRefresh';
import { AuctionActionButton } from './AuctionActionButton';

interface AuctionPublicItemProps {
    auction: AuctionDetails;
    onBidClick: (name: string) => void;
}

export function AuctionPublicItem({ auction, onBidClick }: AuctionPublicItemProps) {
    useAuctionRefresh(auction.metadata);
    const { data: nameRecordData, isLoading: isNameRecordDataLoading } = useNameRecord(
        auction.name,
    );
    const account = useCurrentAccount();

    const normalizedName = normalizeIotaName(auction.name);
    const censoredName = useMemo(() => censorName(normalizedName, FORBIDDEN_LIST), [auction.name]);
    const isCensored = normalizedName !== censoredName;
    const valueToCopy = isCensored ? normalizedName : censoredName;

    const shouldCensor = isCensored;

    const isClaimedAuction = !auction.metadata;
    let auctionDisplayImage = null;

    if (auction.metadata?.nftExpiration) {
        auctionDisplayImage = getNameDisplaySrc(
            auction.name,
            auction.metadata.nftExpiration.getTime(),
        );
    } else if (nameRecordData?.type === 'unavailable' && nameRecordData.nameRecord.expirationDate) {
        auctionDisplayImage = getNameDisplaySrc(
            auction.name,
            nameRecordData.nameRecord.expirationDate.getTime(),
        );
    }

    const auctionStatus = getUserAuctionStatus(auction.metadata, account?.address || '');

    const formattedPrice = auction.metadata
        ? formatNanosToIota(auction.metadata.currentBidNanos ?? BigInt(0), {
              showIotaSymbol: false,
          })
        : null;

    const priceNanos = auction.metadata ? auction.metadata.currentBidNanos : BigInt(0);
    const fiatPrice = useCalculatePriceInFiat(priceNanos);

    if (auction.isLoading || isNameRecordDataLoading) {
        return (
            <Card type={CardType.Filled}>
                <div className="animate-pulse space-y-3">
                    <div className="h-4 rounded bg-gray-700 w-3/4"></div>
                    <div className="h-3 rounded bg-gray-700 w-1/2"></div>
                    <div className="h-3 rounded bg-gray-700 w-full"></div>
                </div>
            </Card>
        );
    }

    return (
        <NameCard
            name={auction.name}
            size="full"
            displaySrc={auctionDisplayImage}
            blurImage={shouldCensor}
            testId="name-card"
        >
            <NameCardBody name={censoredName} valueToCopy={valueToCopy}>
                {auction.hasUserParticipated ? (
                    <div className="absolute top-2 left-2" data-testid="auction-status-badge">
                        <AuctionStatusBadge status={auctionStatus} />
                    </div>
                ) : null}
                {!isClaimedAuction ? (
                    <div className="flex items-center justify-between">
                        <div>
                            <div className="text-label-md text-names-neutral-50">Current Bid</div>
                            <div className="flex mt-1 items-center gap-2">
                                <div className="bg-names-solid-blue rounded-full w-5 h-5 flex items-center justify-center">
                                    <IotaLogoSmall className="w-4 h-4" />
                                </div>
                                <div className="flex flex-wrap items-end gap-1">
                                    <span className="text-body-lg leading-none whitespace-nowrap shrink-0">
                                        {formattedPrice}
                                    </span>
                                    {fiatPrice && (
                                        <span className="text-body-sm text-names-neutral-50 leading-none whitespace-nowrap shrink-0">
                                            ${fiatPrice}
                                        </span>
                                    )}
                                </div>
                            </div>
                        </div>
                        <div>
                            <AuctionActionButton
                                auction={auction}
                                auctionStatus={auctionStatus}
                                onBidClick={onBidClick}
                            />
                        </div>
                    </div>
                ) : (
                    <ClaimedAuctionBody auction={auction} />
                )}
                <AuctionTimeRemaining auction={auction} />
            </NameCardBody>
        </NameCard>
    );
}

function ClaimedAuctionBody({ auction }: { auction: AuctionDetails }) {
    const { network } = useIotaClientContext();
    const name = auction.name;
    const { data: nameRecordData, isLoading: isNameRecordDataLoading } = useNameRecord(name);

    const nftId = nameRecordData?.type === 'unavailable' ? nameRecordData?.nameRecord?.nftId : null;

    function onViewNftClick(e: MouseEvent) {
        if (nftId) {
            e.preventDefault();
            window.open(
                `https://explorer.iota.org/object/${nftId}?network=${network}`,
                '_blank',
                'noopener,noreferrer',
            );
        }
    }

    return (
        <div className="flex items-center justify-between">
            <div>
                <div className="flex mt-1 items-center gap-2">
                    <div className="text-label-md text-names-neutral-50">Claimed</div>
                </div>
            </div>
            <div>
                <Button
                    text="View NFT"
                    type={ButtonType.Outlined}
                    onClick={(e) => onViewNftClick(e)}
                    disabled={!nftId}
                    fullWidth
                    icon={
                        isNameRecordDataLoading ? (
                            <Loader className="animate-spin" data-testid="loading-indicator" />
                        ) : null
                    }
                />
            </div>
        </div>
    );
}

function AuctionTimeRemaining({ auction }: { auction: AuctionDetails }) {
    const timeRemainingMs = getTimeRemaining(auction.metadata);
    const { milliseconds } = useCountdown(timeRemainingMs);

    const formattedTimeRemaining = formatTimeRemaining(milliseconds);

    return (
        <div className="flex items-center justify-between">
            <div>
                <span className="text-label-md text-names-neutral-50">Time left</span>
            </div>
            <div className="flex items-center gap-1" data-testid="auction-time-remaining">
                <Clock className="w-4 h-4 text-names-neutral-50" />
                <span className="text-label-md text-names-neutral-50">
                    {formattedTimeRemaining}
                </span>
            </div>
        </div>
    );
}
