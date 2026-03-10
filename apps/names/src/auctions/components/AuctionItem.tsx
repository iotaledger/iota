// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Card, CardType } from '@iota/apps-ui-kit';
import { normalizeIotaName } from '@iota/iota-names-sdk';

import { NameCard } from '@/components/name-card/NameCard';
import { NameCardBody } from '@/components/name-card/NameCardBody';
import { ExpiryDateIndicator } from '@/components/name-card/NameCardIndicators';

import { getNameDisplaySrc } from '../../lib/utils/displayImage';
import { useAuctionRefresh } from '../hooks/useAuctionRefresh';
import { AuctionDetails } from '../hooks/useAuctions';
import { UserAuctionStatus } from '../lib/utils';
import { AuctionActionButton } from './AuctionActionButton';
import { AuctionStatusBadge } from './AuctionStatusBadge';

interface AuctionItemProps {
    auction: AuctionDetails;
    auctionStatus: UserAuctionStatus;
    onBidClick?: (targetName: string) => void;
}

export function AuctionItem({ auction, auctionStatus, onBidClick }: AuctionItemProps) {
    useAuctionRefresh(auction.metadata);

    const auctionDisplayImage = auction.metadata?.nftExpiration
        ? getNameDisplaySrc(auction.name, auction.metadata.nftExpiration.getTime())
        : null;

    if (!auction.metadata || auction.isLoading) {
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

    if (auction.error || !auction.metadata) {
        return (
            <Card type={CardType.Filled}>
                <div className="text-red-400">
                    <p className="font-medium">{auction.name}</p>
                    <p className="text-sm">Failed to load auction data</p>
                </div>
            </Card>
        );
    }

    return (
        <NameCard name={auction.name} displaySrc={auctionDisplayImage} testId="name-card">
            <NameCardBody name={normalizeIotaName(auction.name)}>
                <div className="flex flex-row items-center justify-between gap-x-xs">
                    <ExpiryDateIndicator auction={auction} />
                    <div data-testid="auction-status-badge">
                        <AuctionStatusBadge status={auctionStatus} />
                    </div>
                </div>

                <div className="min-h-[44px] flex w-full">
                    <AuctionActionButton
                        auction={auction}
                        auctionStatus={auctionStatus}
                        onBidClick={onBidClick}
                    />
                </div>
            </NameCardBody>
        </NameCard>
    );
}
