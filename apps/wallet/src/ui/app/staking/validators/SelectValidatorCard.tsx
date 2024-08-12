// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Content, Menu } from '_app/shared/bottom-menu-layout';
import { Button } from '_app/shared/ButtonUI';
import { Text } from '_app/shared/text';
import { Alert, LoadingIndicator } from '_components';
import { ampli } from '_src/shared/analytics/ampli';
import { calculateStakeShare, formatPercentageDisplay, useGetValidatorsApy } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { ArrowRight16 } from '@iota/icons';
import cl from 'clsx';
import { useMemo, useState } from 'react';

import { ValidatorListItem } from './ValidatorListItem';

type SortKeys = 'name' | 'stakeShare' | 'apy';
const SORT_KEYS: Record<SortKeys, string> = {
    name: 'Name',
    stakeShare: 'Stake Share',
    apy: 'APY',
};

type Validator = {
    name: string;
    address: string;
    apy: number | null;
    isApyApproxZero?: boolean;
    stakeShare: number;
};

export function SelectValidatorCard() {
    const [selectedValidator, setSelectedValidator] = useState<Validator | null>(null);
    const [sortKey, setSortKey] = useState<SortKeys | null>(null);
    const [sortAscending, setSortAscending] = useState(true);
    const { data, isPending, isError } = useIotaClientQuery('getLatestIotaSystemState');

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const selectValidator = (validator: Validator) => {
        setSelectedValidator((state) => (state?.address !== validator.address ? validator : null));
    };

    const handleSortByKey = (key: SortKeys) => {
        if (key === sortKey) {
            setSortAscending(!sortAscending);
        }
        setSortKey(key);
    };

    const totalStake = useMemo(() => {
        if (!data) return 0;
        return data.activeValidators.reduce(
            (acc, curr) => (acc += BigInt(curr.stakingPoolIotaBalance)),
            0n,
        );
    }, [data]);

    const validatorsRandomOrder = useMemo(
        () => [...(data?.activeValidators || [])].sort(() => 0.5 - Math.random()),
        [data?.activeValidators],
    );
    const validatorList = useMemo(() => {
        const sortedAsc = validatorsRandomOrder.map((validator) => {
            const { apy, isApyApproxZero } = rollingAverageApys?.[validator.iotaAddress] ?? {
                apy: null,
            };
            return {
                name: validator.name,
                address: validator.iotaAddress,
                apy,
                isApyApproxZero,
                stakeShare: calculateStakeShare(
                    BigInt(validator.stakingPoolIotaBalance),
                    BigInt(totalStake),
                ),
            };
        });
        if (sortKey) {
            sortedAsc.sort((a, b) => {
                if (sortKey === 'name') {
                    return a[sortKey].localeCompare(b[sortKey], 'en', {
                        sensitivity: 'base',
                        numeric: true,
                    });
                }
                // since apy can be null, fallback to 0
                return (a[sortKey] || 0) - (b[sortKey] || 0);
            });

            return sortAscending ? sortedAsc : sortedAsc.reverse();
        }
        return sortedAsc;
    }, [validatorsRandomOrder, sortAscending, rollingAverageApys, totalStake, sortKey]);

    if (isPending) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isError) {
        return (
            <div className="p-2">
                <Alert>
                    <div className="mb-1 font-semibold">Something went wrong</div>
                </Alert>
            </div>
        );
    }

    return (
        <div className="-my-5 flex h-full w-full flex-col">
            <Content className="flex w-full flex-col items-center">
                <div className="sticky -top-5 z-50 mt-0 flex w-full flex-col items-center bg-white pb-2.5 pt-5">
                    <div className="mb-2 flex w-full items-start">
                        <Text variant="subtitle" weight="medium" color="steel-darker">
                            Sort by:
                        </Text>
                        <div className="ml-2 flex items-center gap-1.5">
                            {Object.entries(SORT_KEYS).map(([key, value]) => {
                                return (
                                    <button
                                        key={key}
                                        className="flex cursor-pointer gap-1 border-0 bg-transparent p-0"
                                        onClick={() => handleSortByKey(key as SortKeys)}
                                    >
                                        <Text
                                            variant="caption"
                                            weight="medium"
                                            color={sortKey === key ? 'hero' : 'steel-darker'}
                                        >
                                            {value}
                                        </Text>
                                        {sortKey === key && (
                                            <ArrowRight16
                                                className={cl(
                                                    'text-hero text-captionSmall font-thin',
                                                    sortAscending ? 'rotate-90' : '-rotate-90',
                                                )}
                                            />
                                        )}
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                    <div className="flex w-full items-start">
                        <Text variant="subtitle" weight="medium" color="steel-darker">
                            Select a validator to start staking IOTA.
                        </Text>
                    </div>
                </div>
                <div className="mt-1 flex w-full flex-1 flex-col items-start">
                    {data &&
                        validatorList.map((validator) => (
                            <div
                                data-testid="validator-list-item"
                                className="relative w-full cursor-pointer"
                                key={validator.address}
                                onClick={() => selectValidator(validator)}
                            >
                                <ValidatorListItem
                                    selected={selectedValidator?.address === validator.address}
                                    validatorAddress={validator.address}
                                    value={formatPercentageDisplay(
                                        !sortKey || sortKey === 'name' ? null : validator[sortKey],
                                        '-',
                                        validator?.isApyApproxZero,
                                    )}
                                />
                            </div>
                        ))}
                </div>
            </Content>
            {selectedValidator && (
                <Menu stuckClass="staked-cta" className="-bottom-5 mx-0 w-full px-0 pb-5">
                    <Button
                        data-testid="select-validator-cta"
                        size="tall"
                        variant="primary"
                        to={`/stake/new?address=${encodeURIComponent(selectedValidator.address)}`}
                        onClick={() =>
                            ampli.selectedValidator({
                                validatorName: selectedValidator.name,
                                validatorAddress: selectedValidator.address,
                                validatorAPY: selectedValidator.apy || 0,
                            })
                        }
                        text="Select Amount"
                        after={<ArrowRight16 />}
                    />
                </Menu>
            )}
        </div>
    );
}
