// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    ArrowLeft,
    ArrowRight,
    FilterList,
    Info,
    Refresh,
    Search,
    Warning,
} from '@iota/apps-ui-icons';
import {
    Button,
    ButtonSegment,
    ButtonSegmentType,
    ButtonSize,
    ButtonType,
    Chip,
    ChipType,
    Dropdown,
    DropdownPosition,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Input,
    InputType,
    ListItem,
    LoadingIndicator,
    SegmentedButton,
    Select,
    SelectSize,
    TablePaginationOptions,
} from '@iota/apps-ui-kit';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useRouter, useSearchParams } from 'next/navigation';
import { useEffect, useRef, useState } from 'react';

import { AuctionBidDialog } from '@/auctions';
import { AuctionPublicItem } from '@/auctions/components/AuctionPublicItem';
import { useAuctions } from '@/auctions/hooks/useAuctions';
import { CardSkeletonLoader } from '@/components/skeletons/CardSkeletonLoader';
import { useRefreshAuctions } from '@/hooks';
import { useDebounce } from '@/hooks/useDebounce';
import { getPaginationPages } from '@/lib/utils';
import { ampli } from '@/lib/utils/analytics/ampli';

import { paramsSchema } from './params';

export type AuctionStatus = 'all' | 'active' | 'finished';

const SORT_OPTIONS = [
    {
        label: 'Bid: low to high',
        sort: 'asc' as const,
        sortBy: 'bid' as const,
    },
    {
        label: 'Bid: high to low',
        sort: 'desc' as const,
        sortBy: 'bid' as const,
    },
    {
        label: 'Name: A-Z',
        sort: 'asc' as const,
        sortBy: 'name' as const,
    },
    {
        label: 'Name: Z-A',
        sort: 'desc' as const,
        sortBy: 'name' as const,
    },
    {
        label: 'Ending: soonest first',
        sort: 'asc' as const,
        sortBy: 'ending' as const,
    },
    {
        label: 'Ending: latest first',
        sort: 'desc' as const,
        sortBy: 'ending' as const,
    },
];

const PAGE_SIZES_RANGE = [10, 20, 50, 100];
const DEBOUNCE_DELAY = 500;

export default function AuctionsPage(): JSX.Element {
    const searchParams = useSearchParams();
    const router = useRouter();
    const currentAddress = useCurrentAccount()?.address;
    const [areFiltersVisible, setAreFiltersVisible] = useState<boolean>(false);
    const [bidDialogName, setBidDialogName] = useState<string | null>(null);
    const dropdownRef = useRef<HTMLDivElement>(null);

    const raw = Object.fromEntries(searchParams.entries());
    const parsed = paramsSchema.safeParse(raw);
    const {
        page,
        status: selectedStatus,
        search: searchQuery,
        size: pageSize,
        sort,
        sortBy,
    } = parsed.success ? parsed.data : paramsSchema.parse({});

    // Close dropdown when clicking outside
    useEffect(() => {
        function handleClickOutside(event: MouseEvent) {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setAreFiltersVisible(false);
            }
        }

        if (areFiltersVisible) {
            document.addEventListener('mousedown', handleClickOutside);
        }

        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
        };
    }, [areFiltersVisible]);

    const debouncedSearchQuery = useDebounce(searchQuery, DEBOUNCE_DELAY);

    useEffect(() => {
        if (debouncedSearchQuery && debouncedSearchQuery.trim()) {
            ampli.appliedAuctionNameFilter({ query: debouncedSearchQuery.trim() });
        }
    }, [debouncedSearchQuery]);

    function setParams(keys: Array<[key: string, value: string | number | boolean]>) {
        const params = new URLSearchParams(searchParams.toString());

        // Always go to page 1 if a param is changed
        // If its changed in the for-loop then it will go to that other page
        params.set('page', '1');

        for (const [key, value] of keys) {
            params.set(key, value.toString());
        }
        router.replace(`?${params}`);
    }

    function setParam(key: string, value: string | number | boolean) {
        setParams([[key, value]]);
    }

    const {
        data: auctions,
        totalItems,
        isLoading,
        error: isAuctionsError,
    } = useAuctions({
        userAddress: currentAddress,
        search: debouncedSearchQuery,
        status: selectedStatus,
        sort,
        sortBy,
        page: page - 1,
        pageSize,
    });

    const { isRefreshing, handleRefresh } = useRefreshAuctions(auctions);

    const typeOptions = [
        {
            label: 'All',
            value: 'all' as AuctionStatus,
        },
        {
            label: 'Active',
            value: 'active' as AuctionStatus,
        },
        {
            label: 'Finished',
            value: 'finished' as AuctionStatus,
        },
    ];

    const infoBox = (() => {
        if (isLoading) {
            return null;
        }

        if (isAuctionsError) {
            return (
                <div className="flex">
                    <InfoBox
                        style={InfoBoxStyle.Elevated}
                        type={InfoBoxType.Error}
                        supportingText={'Failed to load auctions. Please try again later.'}
                        icon={<Warning />}
                    />
                </div>
            );
        }

        if (auctions.length === 0) {
            return (
                <div className="flex">
                    <InfoBox
                        style={InfoBoxStyle.Elevated}
                        type={InfoBoxType.Default}
                        supportingText={'There are no auctions available at the moment.'}
                        icon={<Info />}
                    />
                </div>
            );
        }
    })();

    const totalPages = Math.ceil(totalItems / pageSize);
    const paginationPages = getPaginationPages(page, totalPages, 4);

    const defaultPage = 1;
    const paginationOptions: TablePaginationOptions = {
        hasPrev: page > defaultPage,
        hasNext: page < totalPages,
        onFirst: () => {
            setParam('page', defaultPage);
        },
        onPrev: () => {
            const prevPage = page > defaultPage ? page - 1 : defaultPage;
            setParam('page', prevPage);
        },
        onNext: () => {
            const nextPage = page < totalPages ? page + 1 : page;
            setParam('page', nextPage);
        },
    };

    return (
        <>
            <div className="flex flex-row gap-md items-center pt-[80px] md:pt-0">
                <h2 className="text-headline-md text-names-neutral-92 font-bold leading-[120%] -tracking-[0.4px]">
                    Auctions
                </h2>
                <div className="w-[40px] h-[40px] flex items-center justify-center">
                    {selectedStatus === 'active' || selectedStatus === 'all' ? (
                        <Button
                            type={ButtonType.Outlined}
                            icon={isRefreshing ? <LoadingIndicator size="w-4 h-4" /> : <Refresh />}
                            onClick={handleRefresh}
                            disabled={isRefreshing}
                            testId="refresh-button"
                            aria-label="Refresh"
                        />
                    ) : null}
                </div>
            </div>
            <div className="flex-row md:flex justify-between items-center relative">
                <div className="flex items-center gap-md">
                    <SegmentedButton>
                        {typeOptions.map((option) => (
                            <ButtonSegment
                                key={option.value}
                                type={ButtonSegmentType.Rounded}
                                label={option.label}
                                selected={selectedStatus === option.value}
                                onClick={() => setParam('status', option.value)}
                            />
                        ))}
                    </SegmentedButton>
                    {!isLoading && (
                        <p className="text-label-md whitespace-nowrap text-names-neutral-70">
                            {totalItems} Total
                        </p>
                    )}
                </div>

                <div className="mt-4 md:mt-0 flex gap-4 items-center" ref={dropdownRef}>
                    <div className="w-full max-w-2xl flex flex-col backdrop-blur-md bg-white/5 overflow-hidden [&_*]:!border-transparent rounded-full [&>div]:rounded-full [&_.input-container]:rounded-full">
                        <Input
                            placeholder="Search auction"
                            type={InputType.Text}
                            value={searchQuery}
                            onChange={(e) => setParam('search', e.target.value.toLowerCase())}
                            trailingElement={<Search className="text-names-neutral-92 w-6 h-6" />}
                            autoCapitalize="none"
                        />
                    </div>
                    <Button
                        type={ButtonType.Secondary}
                        icon={<FilterList className="text-names-neutral-92 w-6 h-6" />}
                        onClick={() => setAreFiltersVisible(!areFiltersVisible)}
                        aria-label="Sort options"
                    />
                    {areFiltersVisible && (
                        <div className="absolute right-12 top-0 z-10">
                            <Dropdown>
                                {SORT_OPTIONS.map((option) => (
                                    <ListItem
                                        key={`${option.sort}-${option.sortBy}`}
                                        onClick={() => {
                                            setParams([
                                                ['sort', option.sort],
                                                ['sortBy', option.sortBy],
                                            ]);
                                        }}
                                        hideBottomBorder
                                        isHighlighted={
                                            sort === option.sort && sortBy === option.sortBy
                                        }
                                    >
                                        <span>{option.label}</span>
                                    </ListItem>
                                ))}
                            </Dropdown>
                        </div>
                    )}
                </div>
            </div>
            <div className="flex flex-col flex-1 justify-between">
                <div>
                    {infoBox}
                    {isLoading ? (
                        <div className="flex w-full justify-start mt-8">
                            <CardSkeletonLoader isAuctionCard />
                        </div>
                    ) : (
                        <div className="mt-8 gap-lg w-full flex flex-row items-center flex-wrap justify-center sm:justify-start">
                            {auctions.map((auction) => (
                                <div key={auction.name} className="w-[220px]">
                                    <AuctionPublicItem
                                        auction={auction}
                                        onBidClick={setBidDialogName}
                                    />
                                </div>
                            ))}
                        </div>
                    )}
                </div>
                {paginationPages.length > 0 ? (
                    <div className="mt-8 flex flex-col items-center gap-sm md:relative md:gap-0">
                        <div className="flex flex-1 justify-center">
                            <div className="flex gap-2">
                                <Button
                                    type={ButtonType.Secondary}
                                    size={ButtonSize.Small}
                                    icon={<ArrowLeft />}
                                    disabled={!paginationOptions.hasPrev}
                                    onClick={paginationOptions.onPrev}
                                    aria-label="Previous page"
                                />
                            </div>
                            <div className="flex gap-2 mx-6">
                                {paginationPages.map((pageNumber, index) => {
                                    if (pageNumber === '...') {
                                        return (
                                            <span
                                                key={`ellipsis-${index}`}
                                                className="text-names-neutral-92"
                                            >
                                                {pageNumber}
                                            </span>
                                        );
                                    }

                                    return (
                                        <Chip
                                            key={pageNumber}
                                            type={ChipType.Outline}
                                            selected={page === pageNumber}
                                            onClick={() => setParam('page', pageNumber)}
                                            label={pageNumber.toString()}
                                        />
                                    );
                                })}
                            </div>
                            <div className="flex gap-2">
                                <Button
                                    type={ButtonType.Secondary}
                                    size={ButtonSize.Small}
                                    icon={<ArrowRight />}
                                    disabled={!paginationOptions.hasNext}
                                    onClick={paginationOptions.onNext}
                                    aria-label="Next page"
                                />
                            </div>
                        </div>
                        <div className="md:absolute md:right-0 md:top-1/2 md:-translate-y-1/2">
                            <Select
                                dropdownPosition={DropdownPosition.Top}
                                value={pageSize.toString()}
                                options={PAGE_SIZES_RANGE.map((size) => ({
                                    label: `${size} / page`,
                                    id: size.toString(),
                                }))}
                                size={SelectSize.Small}
                                onValueChange={(e) => {
                                    setParam('size', e);
                                }}
                            />
                        </div>
                    </div>
                ) : null}
            </div>

            {bidDialogName && (
                <AuctionBidDialog
                    name={bidDialogName}
                    closeDialog={() => setBidDialogName(null)}
                    onCompleted={() => {
                        setBidDialogName(null);
                    }}
                />
            )}
        </>
    );
}
