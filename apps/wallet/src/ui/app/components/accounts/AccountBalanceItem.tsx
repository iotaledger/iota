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
    useGetStardustSharedBasicObjects,
    useGetStardustSharedNftObjects,
} from '@iota/core';
import { TriangleDown } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { Badge, BadgeType } from '@iota/apps-ui-kit';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { useGetOwnedObjectsMultipleAddresses } from '../../hooks';

interface AccountBalanceItemProps {
    accounts: SerializedUIAccount[];
    accountIndex: string;
}

const OBJECT_PER_REQ = 1;

export function AccountBalanceItem({
    accounts,
    accountIndex,
}: AccountBalanceItemProps): JSX.Element {
    const addresses = accounts.map(({ address }) => address);

    const balances = accounts.map(({ address }) => ({
        address,
        balance: useBalance(address, { refetchInterval: false }).data,
    }));

    const { data: ownedObjects } = useGetOwnedObjectsMultipleAddresses(
        addresses,
        {
            MatchNone: [
                { StructType: COIN_TYPE },
                { StructType: TIMELOCK_IOTA_TYPE },
                { StructType: TIMELOCK_STAKED_TYPE },
                { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                { StructType: STARDUST_NFT_OUTPUT_TYPE },
            ],
        },
        OBJECT_PER_REQ,
    );

    const { data: vestingObjects } = useGetOwnedObjectsMultipleAddresses(
        addresses,
        {
            MatchAny: [{ StructType: TIMELOCK_IOTA_TYPE }, { StructType: TIMELOCK_STAKED_TYPE }],
        },
        OBJECT_PER_REQ,
    );

    const { data: stardustOwnedObjects } = useGetOwnedObjectsMultipleAddresses(
        addresses,
        {
            MatchAny: [
                { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                { StructType: STARDUST_NFT_OUTPUT_TYPE },
            ],
        },
        OBJECT_PER_REQ,
    );

    const migrationObjects = addresses.map((address) => {
        const stardustSharedBasic = useGetStardustSharedBasicObjects(address, OBJECT_PER_REQ).data;
        const stardustSharedNft = useGetStardustSharedNftObjects(address, OBJECT_PER_REQ).data;
        return (
            !!stardustOwnedObjects?.pages[0][0].data?.length ||
            !!stardustSharedBasic?.length ||
            !!stardustSharedNft?.length
        );
    });

    function getAddressBalance(address: string): string {
        const balanceData = balances.find((b) => b.address === address)?.balance;
        const totalBalance = balanceData?.totalBalance || '0';
        const coinType = balanceData?.coinType || '';
        const [formatted, symbol] = useFormatCoin(BigInt(totalBalance), coinType);
        return `${formatted} ${symbol}`;
    }

    function getSumOfBalances(): string {
        let coinType = '';
        const balance = balances.reduce((acc, { balance }) => {
            const totalBalance = balance?.totalBalance || '0';
            coinType = balance?.coinType || '';
            return (BigInt(acc) + BigInt(totalBalance)).toString();
        }, '0');
        const [formatted, symbol] = useFormatCoin(BigInt(balance), coinType);
        return `${formatted} ${symbol}`;
    }

    const hasAccountAssets = useMemo(() => {
        return ownedObjects?.pages.some((obj) => Boolean(obj[0]?.data?.length));
    }, [ownedObjects]);

    const hasVestingObjects = useMemo(() => {
        return vestingObjects?.pages.some((obj) => Boolean(obj[0]?.data?.length));
    }, [vestingObjects]);

    const hasMigrationObjects = useMemo(() => {
        console.log('migrationObjects', migrationObjects);
        return migrationObjects.some((mig) => mig);
    }, [migrationObjects]);

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
                                {accounts.length} {accounts.length > 1 ? 'addresses' : 'address'}
                            </span>
                        </div>
                    </div>
                    <div className="flex flex-col items-end gap-xxs">
                        <span>{getSumOfBalances()}</span>
                        <div className="flex flex-row gap-xxs">
                            {hasAccountAssets && <Badge type={BadgeType.Neutral} label="Assets" />}
                            {hasVestingObjects && (
                                <Badge type={BadgeType.Neutral} label="Vesting" />
                            )}
                            {hasMigrationObjects && (
                                <Badge type={BadgeType.Neutral} label="Migration" />
                            )}
                        </div>
                    </div>
                </div>
            )}
        >
            <div className="flex flex-col gap-y-sm p-sm pl-lg text-body-md text-neutral-10 dark:text-neutral-92">
                {accounts.map(({ address, id }) => (
                    <div className="flex w-full flex-row justify-between" key={id}>
                        <span>{formatAddress(address)}</span>
                        <span>{getAddressBalance(address)}</span>
                    </div>
                ))}
            </div>
        </Collapsible>
    );
}
