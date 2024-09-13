// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getCoinSymbol } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type CoinBalance } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { LoadingIndicator } from '@iota/ui';
import { FilterList, Warning } from '@iota/ui-icons';
import { useMemo, useState } from 'react';
import OwnedCoinView from './OwnedCoinView';
import { useRecognizedPackages } from '~/hooks/useRecognizedPackages';
import {
    Button,
    ButtonType,
    Dropdown,
    DropdownPosition,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    ListItem,
    Select,
    Title,
} from '@iota/apps-ui-kit';
import { Pagination } from '../ui';

export type CoinBalanceVerified = CoinBalance & {
    isRecognized?: boolean;
};

enum CoinFilter {
    All = 'allBalances',
    Recognized = 'recognizedBalances',
    Unrecognized = 'unrecognizedBalances',
}

interface OwnerCoinsProps {
    id: string;
}
export function OwnedCoins({ id }: OwnerCoinsProps): JSX.Element {
    const [currentSlice, setCurrentSlice] = useState(1);
    const [limit, setLimit] = useState(20);
    const [filterValue, setFilterValue] = useState(CoinFilter.All);
    const { isPending, data, isError } = useIotaClientQuery('getAllBalances', {
        owner: normalizeIotaAddress(id),
    });
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

    const filterOptions: FilterOption[] = useMemo(
        () => [
            { label: 'All', onClick: () => setFilterValue(CoinFilter.All) },
            {
                label: `Recognized`,
                counter: balances.recognizedBalances.length,
                isDisabled: !balances.recognizedBalances.length,
                onClick: () => setFilterValue(CoinFilter.Recognized),
            },
            {
                label: `Unrecognized`,
                counter: balances.unrecognizedBalances.length,
                isDisabled: !balances.unrecognizedBalances.length,
                onClick: () => setFilterValue(CoinFilter.Unrecognized),
            },
        ],
        [balances],
    );

    const hasCoinsBalance = balances.allBalances.length > 0;
    const coinBalanceHeader =
        `${balances.allBalances.length ?? 0} Coin` + (balances.allBalances.length !== 1 ? 's' : '');
    const displayedBalances = useMemo(() => balances[filterValue], [balances, filterValue]);

    if (isError) {
        return (
            <div className="pt-2 font-sans font-semibold text-issue-dark">Failed to load Coins</div>
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
                    <Title
                        title={coinBalanceHeader}
                        trailingElement={
                            hasCoinsBalance && <CoinsFilter filterOptions={filterOptions} />
                        }
                    />
                    <div className="px-md--rs">
                        {filterValue === CoinFilter.Unrecognized && (
                            <InfoBox
                                icon={<Warning />}
                                supportingText="These coins have not been recognized by the Iota Foundation."
                                type={InfoBoxType.Default}
                                style={InfoBoxStyle.Default}
                            />
                        )}
                    </div>
                    {hasCoinsBalance ? (
                        <>
                            <div className="overflow-y-auto p-sm--rs">
                                <CoinList coins={visibleCoins} id={id} />
                            </div>

                            {displayedBalances.length > limit && (
                                <div className="flex flex-col justify-between gap-2 px-sm--rs py-xs--rs md:flex-row">
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
                                        <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                                            {`Showing `}
                                            {(currentSlice - 1) * limit + 1}-
                                            {currentSlice * limit > displayedBalances.length
                                                ? displayedBalances.length
                                                : currentSlice * limit}
                                        </span>
                                        <Select
                                            dropdownPosition={DropdownPosition.Top}
                                            value={limit.toString()}
                                            options={[
                                                { label: '20 Per Page', id: '20' },
                                                { label: '40 Per Page', id: '40' },
                                                { label: '60 Per Page', id: '60' },
                                            ]}
                                            onValueChange={(value) => {
                                                setLimit(Number(value));
                                                setCurrentSlice(1);
                                            }}
                                        />
                                    </div>
                                </div>
                            )}
                        </>
                    ) : (
                        <div className="flex h-20 items-center justify-center md:h-coinsAndAssetsContainer">
                            <span className="flex flex-row items-center gap-x-xs text-neutral-40 dark:text-neutral-60">
                                No Coins Owned
                            </span>
                        </div>
                    )}
                </div>
            )}
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
            <Button type={ButtonType.Ghost} onClick={toggleFilterDropdown} icon={<FilterList />} />
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

interface CoinListProps {
    coins: CoinBalanceVerified[];
    id: string;
}

function CoinList({ coins, id }: CoinListProps) {
    return (
        <>
            {coins.map((coin, index) => (
                <OwnedCoinView key={`${coin.coinType}-${index}`} coin={coin} id={id} />
            ))}
        </>
    );
}
