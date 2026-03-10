// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoSmall } from '@iota/apps-ui-icons';
import { normalizeIotaName } from '@iota/iota-names-sdk';
import clsx from 'clsx';

import { useCalculatePriceInFiat } from '@/hooks';
import { formatNanosToIota } from '@/lib/utils';

import { BG_COLORS, TEXT_COLORS } from './constants';
import { NameAvailabilityStatus } from './enums';

interface NamePurchaseCardProps {
    name: string;
    statusMessage?: string;
    priceNanos?: number | bigint;
    status: NameAvailabilityStatus;
    disableStatusHoverEffect?: boolean;
    testId?: string;
}

export function NamePurchaseCard({
    name,
    statusMessage,
    priceNanos,
    status,
    children,
    disableStatusHoverEffect,
    testId,
}: React.PropsWithChildren<NamePurchaseCardProps>): React.JSX.Element {
    const bgCard = BG_COLORS[status];

    const textColorStatus = TEXT_COLORS[status];
    const fiatPrice = useCalculatePriceInFiat(priceNanos || 0);
    const formattedPrice = priceNanos
        ? formatNanosToIota(priceNanos, {
              showIotaSymbol: false,
          })
        : undefined;

    const isAvailable = status === NameAvailabilityStatus.Available;

    const [_, nameWithOutAt] = normalizeIotaName(name).split('@');
    return (
        <div
            className={clsx(
                'group relative w-full flex flex-col justify-between rounded-2xl p-[1px] gap-y-sm ',
                status !== NameAvailabilityStatus.Unavailable && 'hover:bg-names-gradient-primary',
            )}
            data-testid={testId}
        >
            <div
                className={clsx(
                    'group flex h-full w-full flex-col justify-between rounded-[15px] p-md--rs gap-y-xs md:gap-y-sm',
                    bgCard,
                )}
            >
                <div
                    className="flex text-headline-sm sm:text-headline-md gap-xxs"
                    data-testid="name-purchase-card-name"
                >
                    <span className="text-names-neutral-50">@</span>
                    <h2 className={clsx('truncate w-full overflow-hidden', textColorStatus)}>
                        {nameWithOutAt}
                    </h2>
                </div>
                <div className="flex flex-col sm:flex-row sm:justify-between sm:space-x-4 h-auto w-full gap-lg">
                    <div className="flex flex-row gap-2 text-body-md items-center">
                        <div
                            className={clsx('text-body-md capitalize', textColorStatus)}
                            data-testid="name-purchase-card-status"
                        >
                            {status}
                        </div>
                        {statusMessage && (
                            <p
                                data-testid="name-purchase-card-status-message"
                                className={clsx(
                                    'text-names-neutral-70 transition-opacity duration-100',
                                    isAvailable
                                        ? 'opacity-100'
                                        : disableStatusHoverEffect
                                          ? 'opacity-100'
                                          : 'opacity-0 group-hover:opacity-100',
                                )}
                            >
                                {statusMessage}
                            </p>
                        )}
                    </div>
                    <div className="flex flex-wrap sm:flex-row gap-md justify-end">
                        {priceNanos && (
                            <div className="flex flex-col items-start bg-names-neutral-12 py-sm pr-sm pl-xs rounded-[32px]">
                                <div className="flex items-center gap-xxs">
                                    <IotaLogoSmall className="w-5 h-5 bg-iota-primary-30 rounded-full p-0.5" />
                                    <div className="items-baseline gap-xxxs flex">
                                        <p className="text-body-lg text-names-neutral-92 flex items-baseline gap-xxs font-bold">
                                            {formattedPrice}
                                        </p>
                                        {fiatPrice && (
                                            <span className="text-label-md text-names-neutral-50 items-center flex">
                                                ${fiatPrice}
                                            </span>
                                        )}
                                    </div>
                                </div>
                            </div>
                        )}

                        <div className="flex flex-col sm:flex-row w-0 group-hover:w-auto group-hover:min-w-[80px] whitespace-nowrap overflow-hidden [&>*]:flex-1">
                            {children}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
