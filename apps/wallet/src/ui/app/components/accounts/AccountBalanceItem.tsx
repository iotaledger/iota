// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import {
    COIN_TYPE,
    Collapsible,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    TIMELOCK_IOTA_TYPE,
    TIMELOCK_STAKED_TYPE,
    useBalance,
    useFormatCoin,
    useGetOwnedObjects,
    useStardustIndexerClientContext,
} from '@iota/core';
import { TriangleDown } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { Badge, BadgeType } from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';

interface AccountBalanceItemProps {
    accounts: SerializedUIAccount[];
    accountIndex: string;
}

export function AccountBalanceItem({
    accounts,
    accountIndex,
}: AccountBalanceItemProps): JSX.Element {

    function getAddressBalance(address: string): string {
        const { data: balance } = useBalance(address, {
            refetchInterval: false,
        });
        const totalBalance = balance?.totalBalance || '0';
        const coinType = balance?.coinType;
        const [formatted, symbol] = useFormatCoin(BigInt(totalBalance), coinType);
        return `${formatted} ${symbol}`;
    }

    function getSumOfBalances(): string {
        let coinType = '';
        const balance = accounts.reduce((acc, { address }) => {
            const { data: balance } = useBalance(address, {
                refetchInterval: false,
            });
            const totalBalance = balance?.totalBalance || '0';
            coinType = balance?.coinType || '';
            return (BigInt(acc) + BigInt(totalBalance)).toString();
        }, '0');
        const [formatted, symbol] = useFormatCoin(BigInt(balance), coinType);
        return `${formatted} ${symbol}`;
    }

    function hasAccountAssets(): boolean {
        return accounts.some(({ address }) => {
            const { data: ownedAssets } = useGetOwnedObjects(
                address,
                {
                    MatchAny: [
                        { StructType: COIN_TYPE },
                        { StructType: TIMELOCK_IOTA_TYPE },
                        { StructType: TIMELOCK_STAKED_TYPE },
                        { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                        { StructType: STARDUST_NFT_OUTPUT_TYPE },
                    ],
                },
                1,
            );
            return (ownedAssets && ownedAssets?.pages?.[0]?.data?.length > 0) ?? false;
        });
    }

    function hasSupplyIncreaseVestingObjects(): boolean {
        return accounts.some(({ address }) => {
            const { data: supplyIncreaseVestingObjects } = useGetOwnedObjects(
                address,
                {
                    MatchAny: [
                        { StructType: TIMELOCK_IOTA_TYPE },
                        { StructType: TIMELOCK_STAKED_TYPE },
                    ],
                },
                1,
            );
            return (
                (supplyIncreaseVestingObjects &&
                    supplyIncreaseVestingObjects?.pages?.[0]?.data?.length > 0) ??
                false
            );
        });
    }

    function hasMigrationObjects(): boolean {
        let containsMigrationObjects = false;
        return accounts.some(({ address }) => {
            const { data: legacyObjects } = useGetOwnedObjects(
                address,
                {
                    MatchAny: [
                        { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                        { StructType: STARDUST_NFT_OUTPUT_TYPE },
                    ],
                },
                1,
            );
            containsMigrationObjects = !!legacyObjects?.pages?.[0]?.data?.length;
            if (!legacyObjects || legacyObjects?.pages?.[0]?.data?.length === 0) {
                const { stardustIndexerClient } = useStardustIndexerClientContext();

                const indexedBasicOutputs = stardustIndexerClient?.getBasicOutputs(address, {
                    limit: 1,
                });

                const indexedNftOutputs = stardustIndexerClient?.getNftOutputs(address, {
                    limit: 1,
                });

                containsMigrationObjects = !!indexedBasicOutputs || !!indexedNftOutputs;
            }

            return containsMigrationObjects;
        });
    }

    return (
        <Collapsible
            defaultOpen
            hideArrow
            render={({ isOpen }) => (
                <div className="relative flex min-h-[52px] w-full items-center justify-between gap-1 py-2 pl-1 pr-sm text-neutral-10 dark:text-neutral-92">
                    <div className="flex items-center gap-xxs">
                        <TriangleDown
                            className={clsx(
                                'h-5 w-5 ',
                                isOpen
                                    ? 'rotate-0 transition-transform ease-linear'
                                    : '-rotate-90 transition-transform ease-linear',
                            )}
                        />
                        <div className="flex flex-col items-start gap-xxs">
                            <div className="text-title-md">Wallet {Number(accountIndex) + 1}</div>
                            <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                                {accounts.length} addresses
                            </span>
                        </div>
                    </div>
                    <div className="flex flex-col items-end gap-xxs">
                        <span>{getSumOfBalances()}</span>
                        <div className="flex flex-row gap-xxs">
                            {hasAccountAssets() && (
                                <Badge type={BadgeType.Neutral} label="Assets" />
                            )}
                            {hasSupplyIncreaseVestingObjects() && (
                                <Badge type={BadgeType.Neutral} label="Vesting" />
                            )}
                            {hasMigrationObjects() && (
                                <Badge type={BadgeType.Neutral} label="Migration" />
                            )}
                        </div>
                    </div>
                </div>
            )}
        >
            <div className="flex flex-col gap-y-sm p-sm pl-lg text-body-md text-neutral-10 dark:text-neutral-92">
                {accounts.map(({ address }) => (
                    <div className="flex w-full flex-row justify-between">
                        <span>{formatAddress(address)}</span>
                        <span>{getAddressBalance(address)}</span>
                    </div>
                ))}
            </div>
        </Collapsible>
    );
}
