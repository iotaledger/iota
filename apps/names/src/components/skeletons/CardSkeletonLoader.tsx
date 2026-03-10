// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Skeleton } from '@iota/apps-ui-kit';
import cx from 'clsx';

interface skeletonCardProps {
    isAuctionCard?: boolean;
}

export function CardSkeletonLoader({ isAuctionCard }: skeletonCardProps) {
    return (
        <div className="flex flex-wrap gap-6">
            {Array.from({ length: 10 }).map((_, i) => (
                <div
                    key={i}
                    className={cx(
                        'flex flex-col w-[220px] rounded-xl',
                        isAuctionCard ? 'h-[414px]' : 'h-[363px]',
                    )}
                >
                    <Skeleton className="w-full h-[220px] !rounded-xl" />
                    {isAuctionCard ? (
                        <div className="flex flex-col p-md gap-xs name-card-shadow flex-1">
                            <Skeleton className="!w-2/4 h-4 !rounded-lg" />
                            <div className="flex justify-between items-center">
                                <Skeleton className="!w-1/4 h-4 !rounded-lg" />
                                <Skeleton className="!w-14 h-9 !rounded-full" />
                            </div>
                            <Skeleton className="!w-2/4 h-4 !rounded-lg" />
                            <Skeleton className="!w-full h-[2px] !rounded-lg my-4" />
                            <div className="flex justify-between items-center">
                                <Skeleton className="!w-1/4 h-4 !rounded-lg" />
                                <Skeleton className="!w-14 h-4 !rounded-lg" />
                            </div>
                        </div>
                    ) : (
                        <div className="flex flex-col p-md gap-md name-card-shadow w-full flex-1">
                            <Skeleton className="!w-2/4 h-4 !rounded-lg" />
                            <Skeleton className="!w-1/4 h-3 !rounded-lg" />
                            <Skeleton className="!w-full h-9 !rounded-full" />
                        </div>
                    )}
                </div>
            ))}
        </div>
    );
}
