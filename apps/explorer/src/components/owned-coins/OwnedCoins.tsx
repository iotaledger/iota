// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getCoinSymbol, useGetAllBalances, useRecognizedPackages } from '@iota/core';
import { type CoinBalance } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { FilterList, Warning, SortByDown, SortByUp, SortByDefault } from '@iota/apps-ui-icons';
import { useMemo, useState } from 'react';
import { OwnedCoinView } from './OwnedCoinView';
import {
    Button,
    ButtonType,
    Dropdown,
    DropdownPosition,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    ListItem,
    LoadingIndicator,
    Select,
    SelectSize,
    Title,
} from '@iota/apps-ui-kit';
import { Pagination } from '../ui';
import { PAGE_SIZES_RANGE_20_60 } from '~/lib/constants';

export type CoinBalanceVerified = CoinBalance & {
    isRecognized?: boolean;
};

enum CoinFilter {
    All = 'allBalances',
    Recognized = 'recognizedBalances',
    Unrecognized = 'unrecognizedBalances',
}

export enum SortField {
    Id = 'id',
    Balance = 'balance',
}

export enum SortOrder {
    Asc = 'asc',
    Desc = 'desc',
}

interface OwnerCoinsProps {
    id: string;
}

export function OwnedCoins({ id }: OwnerCoinsProps): JSX.Element {
    const [currentSlice, setCurrentSlice] = useState(1);
    const [limit, setLimit] = useState(20);
    const [filterValue, setFilterValue] = useState(CoinFilter.All);
    const [sortField, setSortField] = useState(SortField.Id);
    const [sortOrder, setSortOrder] = useState(SortOrder.Desc);

    const owner = normalizeIotaAddress(id);
    const { isPending, data, isError } = useGetAllBalances(owner);
    const recognizedPackages = useRecognizedPackages();

    const balances: Record<CoinFilter, CoinBalanceVerified[]> = useMemo(() => {
        const balanceData = data?.reduce(
            (acc, coinBalance) => {
                if (recognizedPackages.includes(coinBalance.coinType.split('::')[0])) {
                    acc.recognizedBalances.push({
                        ...coinBalance,
                        isRecognized: true,
                    });
                } else {
                    acc.unrecognizedBalances.push({ ...coinBalance, isRecognized: false });
                }
                return acc;
            },
            {
                recognizedBalances: [] as CoinBalanceVerified[],
                unrecognizedBalances: [] as CoinBalanceVerified[],
            },
        ) ?? { recognizedBalances: [], unrecognizedBalances: [] };

        const recognizedBalances = balanceData.recognizedBalances.sort((a, b) => {
            // Make sure IOTA always comes first
            if (getCoinSymbol(a.coinType) === 'IOTA') {
                return -1;
            } else if (getCoinSymbol(b.coinType) === 'IOTA') {
                return 1;
            } else {
                return getCoinSymbol(a.coinType).localeCompare(
                    getCoinSymbol(b.coinType),
                    undefined,
                    {
                        sensitivity: 'base',
                    },
                );
            }
        });

        return {
            recognizedBalances,
            unrecognizedBalances: balanceData.unrecognizedBalances.sort((a, b) =>
                getCoinSymbol(a.coinType)!.localeCompare(getCoinSymbol(b.coinType)!, undefined, {
                    sensitivity: 'base',
                }),
            ),
            allBalances: balanceData.recognizedBalances.concat(balanceData.unrecognizedBalances),
        };
    }, [data, recognizedPackages]);

    function handleFilterClick(filterValue: CoinFilter) {
        setFilterValue(filterValue);
        setCurrentSlice(1);
    }

    function handleSortChange(field: SortField, order: SortOrder) {
        setSortField(field);
        setSortOrder(order);
    }

    const filterOptions: FilterOption[] = useMemo(
        () => [
            {
                label: 'All',
                counter: balances.allBalances.length,
                onClick: () => handleFilterClick(CoinFilter.All),
            },
            {
                label: `Recognized`,
                counter: balances.recognizedBalances.length,
                isDisabled: !balances.recognizedBalances.length,
                onClick: () => handleFilterClick(CoinFilter.Recognized),
            },
            {
                label: `Unrecognized`,
                counter: balances.unrecognizedBalances.length,
                isDisabled: !balances.unrecognizedBalances.length,
                onClick: () => handleFilterClick(CoinFilter.Unrecognized),
            },
        ],
        [balances],
    );

    const hasCoinsBalance = balances.allBalances.length > 0;
    const displayedBalances = useMemo(() => balances[filterValue], [balances, filterValue]);
    const coinBalanceHeader =
        `${displayedBalances.length ?? 0} Coin` + (displayedBalances.length !== 1 ? 's' : '');

    if (isError) {
        return (
            <div className="p-sm--rs">
                <InfoBox
                    title="Error"
                    supportingText="Failed to load Coins"
                    icon={<Warning />}
                    type={InfoBoxType.Error}
                    style={InfoBoxStyle.Default}
                />
            </div>
        );
    }

    const visibleCoins = displayedBalances.slice((currentSlice - 1) * limit, currentSlice * limit);

    return (
        <div className="h-full w-full grow">
            {isPending ? (
                <div className="m-auto flex h-full w-full justify-center text-white">
                    <LoadingIndicator />
                </div>
            ) : (
                <div className="flex h-full flex-col">
                    <div className="flex flex-col justify-center sm:min-h-[72px]">
                        <Title
                            title={coinBalanceHeader}
                            trailingElement={
                                hasCoinsBalance && (
                                    <div className="flex items-center gap-xs">
                                        <SortDropdown
                                            sortField={sortField}
                                            sortOrder={sortOrder}
                                            onSortChange={handleSortChange}
                                        />
                                        <CoinsFilter filterOptions={filterOptions} />
                                    </div>
                                )
                            }
                        />
                    </div>
                    {hasCoinsBalance ? (
                        <>
                            <div className="relative overflow-y-auto p-sm--rs pt-0">
                                {filterValue === CoinFilter.Unrecognized && (
                                    <div className="sticky top-0 z-[1] bg-iota-neutral-100 p-sm dark:bg-iota-neutral-10">
                                        <InfoBox
                                            icon={<Warning />}
                                            supportingText="These coins have not been recognized by the IOTA Foundation."
                                            type={InfoBoxType.Warning}
                                            style={InfoBoxStyle.Default}
                                        />
                                    </div>
                                )}
                                <CoinList
                                    coins={visibleCoins}
                                    id={id}
                                    sortField={sortField}
                                    sortOrder={sortOrder}
                                />
                            </div>

                            {displayedBalances.length > limit && (
                                <div className="flex flex-row flex-wrap items-center justify-between gap-xs px-sm--rs py-sm--rs">
                                    <Pagination
                                        hasFirst={currentSlice !== 1}
                                        onNext={() => setCurrentSlice(currentSlice + 1)}
                                        hasNext={
                                            currentSlice !==
                                            Math.ceil(displayedBalances.length / limit)
                                        }
                                        hasPrev={currentSlice !== 1}
                                        onPrev={() => setCurrentSlice(currentSlice - 1)}
                                        onFirst={() => setCurrentSlice(1)}
                                    />
                                    <div className="flex items-center gap-3">
                                        <span className="shrink-0 text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                            Showing {(currentSlice - 1) * limit + 1}-
                                            {currentSlice * limit > displayedBalances.length
                                                ? displayedBalances.length
                                                : currentSlice * limit}
                                        </span>
                                        <Select
                                            dropdownPosition={DropdownPosition.Top}
                                            value={limit.toString()}
                                            options={PAGE_SIZES_RANGE_20_60.map((size) => ({
                                                label: `${size} / page`,
                                                id: size.toString(),
                                            }))}
                                            onValueChange={(value) => {
                                                setLimit(Number(value));
                                                setCurrentSlice(1);
                                            }}
                                            size={SelectSize.Small}
                                        />
                                    </div>
                                </div>
                            )}
                        </>
                    ) : (
                        <NoObjectsOwnedMessage objectType="Coins" />
                    )}
                </div>
            )}
        </div>
    );
}

interface NoObjectsOwnedMessageProps {
    objectType: string;
}
export function NoObjectsOwnedMessage({
    objectType,
}: NoObjectsOwnedMessageProps): React.JSX.Element {
    return (
        <div className="md:h-coinsAndAssetsContainer flex h-full items-center justify-center">
            <span className="flex flex-row items-center gap-x-xs text-iota-neutral-40 dark:text-iota-neutral-60">
                No {objectType} Owned
            </span>
        </div>
    );
}

interface FilterOption {
    label: string;
    isDisabled?: boolean;
    counter?: number;
    onClick: () => void;
}

interface CoinsFilterProps {
    filterOptions: FilterOption[];
}

function CoinsFilter({ filterOptions }: CoinsFilterProps) {
    const [areFiltersVisible, setAreFiltersVisible] = useState<boolean>(false);

    function toggleFilterDropdown() {
        setAreFiltersVisible(!areFiltersVisible);
    }

    return (
        <div className="relative z-10">
            <Button
                type={ButtonType.Ghost}
                onClick={toggleFilterDropdown}
                icon={<FilterList />}
                aria-label="Filter coins"
            />
            {areFiltersVisible && (
                <div className="absolute right-0">
                    <Dropdown>
                        {filterOptions.map(({ onClick, counter, label, isDisabled }, index) => (
                            <ListItem
                                isDisabled={isDisabled}
                                key={index}
                                onClick={() => {
                                    onClick();
                                    toggleFilterDropdown();
                                }}
                                hideBottomBorder
                            >
                                <div className="flex w-full flex-row gap-x-md">
                                    <span>{label}</span>
                                    {counter && (
                                        <span className="ml-auto tabular-nums">{counter}</span>
                                    )}
                                </div>
                            </ListItem>
                        ))}
                    </Dropdown>
                </div>
            )}
        </div>
    );
}

interface SortDropdownProps {
    sortField: SortField;
    sortOrder: SortOrder;
    onSortChange: (field: SortField, order: SortOrder) => void;
}

function SortDropdown({ sortField, sortOrder, onSortChange }: SortDropdownProps) {
    const [isSortVisible, setIsSortVisible] = useState<boolean>(false);

    function toggleSortDropdown() {
        setIsSortVisible(!isSortVisible);
    }

    function handleSortClick(field: SortField) {
        if (field === sortField) {
            onSortChange(field, sortOrder === SortOrder.Asc ? SortOrder.Desc : SortOrder.Asc);
        } else {
            const defaultOrder = field === SortField.Balance ? SortOrder.Desc : SortOrder.Asc;
            onSortChange(field, defaultOrder);
        }
        toggleSortDropdown();
    }

    const sortOptions = [
        { field: SortField.Id, label: 'Object ID' },
        { field: SortField.Balance, label: 'Balance' },
    ];

    return (
        <div className="relative z-10">
            <Button
                type={ButtonType.Ghost}
                onClick={toggleSortDropdown}
                icon={<SortByDefault />}
                text="Sort by"
                iconAfterText
            />
            {isSortVisible && (
                <div className="absolute right-0 min-w-[150px]">
                    <Dropdown>
                        {sortOptions.map(({ field, label }, index) => {
                            const isActive = sortField === field;
                            let currentIcon = null;

                            if (isActive) {
                                const isDefaultSorting =
                                    (field === SortField.Id && sortOrder === SortOrder.Asc) ||
                                    (field === SortField.Balance && sortOrder === SortOrder.Desc);
                                currentIcon = isDefaultSorting ? <SortByUp /> : <SortByDown />;
                            }

                            return (
                                <div
                                    key={index}
                                    className={
                                        isActive
                                            ? 'bg-iota-neutral-100 dark:bg-iota-neutral-10'
                                            : ''
                                    }
                                >
                                    <ListItem
                                        onClick={() => handleSortClick(field)}
                                        hideBottomBorder
                                    >
                                        <div className="flex w-full flex-row items-center justify-between gap-x-md">
                                            <span>{label}</span>
                                            {currentIcon && (
                                                <span className="ml-auto">{currentIcon}</span>
                                            )}
                                        </div>
                                    </ListItem>
                                </div>
                            );
                        })}
                    </Dropdown>
                </div>
            )}
        </div>
    );
}

interface CoinListProps {
    coins: CoinBalanceVerified[];
    id: string;
    sortField: SortField;
    sortOrder: SortOrder;
}

function CoinList({ coins, id, sortField, sortOrder }: CoinListProps) {
    return (
        <div className="flex max-h-[400px] w-full flex-col gap-xxs md:max-h-[650px]">
            {coins.map((coin, index) => (
                <OwnedCoinView
                    key={`${coin.coinType}-${index}`}
                    coin={coin}
                    id={id}
                    sortField={sortField}
                    sortOrder={sortOrder}
                />
            ))}
        </div>
    );
}
