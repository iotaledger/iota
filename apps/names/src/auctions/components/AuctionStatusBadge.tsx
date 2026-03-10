// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType } from '@iota/apps-ui-kit';

import { UserAuctionStatus } from '../lib/utils';

interface AuctionStatusBadgeProps {
    status: UserAuctionStatus;
}

export function AuctionStatusBadge({ status }: AuctionStatusBadgeProps) {
    const getStatusConfig = (status: UserAuctionStatus) => {
        switch (status) {
            case 'top_bidder':
                return {
                    label: 'Top Bidder',
                    type: BadgeType.Success,
                };
            case 'outbid':
                return {
                    label: 'Outbid',
                    type: BadgeType.Warning,
                };
            case 'claimable':
            case 'winner':
                return {
                    label: 'Winner',
                    type: BadgeType.PrimarySolid,
                };
            case 'lost':
                return {
                    label: 'Lost',
                    type: BadgeType.Neutral,
                };
            case 'unknown':
            default:
                return {
                    label: 'Unknown',
                    type: BadgeType.Neutral,
                };
        }
    };

    const { label, type } = getStatusConfig(status);

    return <Badge type={type} label={label} />;
}
