// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import {
    haveSupplyIncreaseLabel,
    COIN_TYPE,
    Collapsible,
    IOTA_COIN_METADATA,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    TIMELOCK_IOTA_TYPE,
    TIMELOCK_STAKED_TYPE,
    useBalance,
    useFormatCoin,
} from '@iota/core';
import { TriangleDown } from '@iota/apps-ui-icons';
import clsx from 'clsx';
import { Badge, BadgeType } from '@iota/apps-ui-kit';
import { formatAddress, IOTA_TYPE_ARG, formatBalance } from '@iota/iota-sdk/utils';
import {
    useGetOwnedObjectsMultipleAddresses,
    useGetSharedObjectsMultipleAddresses,
} from '../../hooks';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useIotaClientContext } from '@iota/dapp-kit';

interface AccountBalanceItemProps {
    accounts: SerializedUIAccount[];
    accountIndex: string;
}

export function AccountBalanceItem({
    accounts,
    accountIndex,
}: AccountBalanceItemProps): JSX.Element {
    const queryClient = useQueryClient();
    const iotaContext = useIotaClientContext();

    const addresses = accounts.map(({ address }) => address);

    const { data: sumOfBalances } = useQuery({
        queryKey: ['getBalance', ...addresses],
        async queryFn() {
            return await Promise.all(
                addresses.map(async (address) => {
                    const params = {
                        coinType: IOTA_TYPE_ARG,
                        owner: address!,
                    };
                    return queryClient.ensureQueryData({
                        queryKey: [iotaContext.network, 'getBalance', params],
                        queryFn: () => iotaContext.client.getBalance(params),
                    });
                }),
            );
        },
        select(balances) {
            const balance = balances.reduce((acc, { totalBalance }) => {
                return BigInt(acc) + BigInt(totalBalance);
            }, BigInt(0));
            const formattedAmount = formatBalance(balance, IOTA_COIN_METADATA.decimals);
            return `${formattedAmount} ${IOTA_COIN_METADATA.symbol}`;
        },
        gcTime: 0,
        staleTime: 0,
    });

    return (
        <Collapsible
            defaultOpen
            hideArrow
            render={({ isOpen }) => (
                <div className="relative flex min-h-[52px] w-full items-center justify-between gap-1 py-2 pl-1 pr-sm text-iota-neutral-10 dark:text-iota-neutral-92">
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
                            <span className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                {accounts.length} {accounts.length > 1 ? 'addresses' : 'address'}
                            </span>
                        </div>
                    </div>

                    <span>{sumOfBalances}</span>
                </div>
            )}
        >
            <div className="flex flex-col gap-y-sm p-sm pl-lg text-body-md text-iota-neutral-10 dark:text-iota-neutral-92">
                {accounts.map(({ address, id }) => (
                    <AddressItem key={id} address={address} />
                ))}
            </div>
        </Collapsible>
    );
}

export function AddressItem({ address }: { address: string }) {
    const { data: balance } = useBalance(address);
    const [formatted, symbol] = useFormatCoin({
        balance: BigInt(balance?.totalBalance ?? 0),
        coinType: balance?.coinType ?? '',
    });
    const amountLabel = `${formatted} ${symbol}`;

    const { data: ownedObjects } = useGetOwnedObjectsMultipleAddresses(
        [address],
        {
            MatchNone: [
                { StructType: COIN_TYPE },
                { StructType: TIMELOCK_IOTA_TYPE },
                { StructType: TIMELOCK_STAKED_TYPE },
                { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                { StructType: STARDUST_NFT_OUTPUT_TYPE },
            ],
        },
        1,
    );
    const hasAssets = ownedObjects?.pages?.some((pg) => pg.some((d) => d.data.length > 0)) ?? false;

    const { data: vestingObjects } = useGetOwnedObjectsMultipleAddresses(
        [address],
        {
            MatchAny: [{ StructType: TIMELOCK_IOTA_TYPE }, { StructType: TIMELOCK_STAKED_TYPE }],
        },
        1,
    );
    const hasVesting = haveSupplyIncreaseLabel(vestingObjects?.pages?.flat() ?? []);

    const { data: stardustOwned } = useGetOwnedObjectsMultipleAddresses(
        [address],
        {
            MatchAny: [
                { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                { StructType: STARDUST_NFT_OUTPUT_TYPE },
            ],
        },
        1,
    );
    const { data: stardustShared } = useGetSharedObjectsMultipleAddresses([address], 1);

    const hasMigration =
        (stardustOwned?.pages?.some((pg) => pg.some((d) => d.data.length > 0)) ?? false) ||
        (stardustShared?.pages?.some((pg) =>
            pg.some((d) => d.nftOutputs.length > 0 || d.basicOutputs.length > 0),
        ) ??
            false);

    return (
        <div className="flex w-full flex-col items-center gap-xxs py-xxs">
            <div className="flex w-full flex-row justify-between">
                <span data-amp-mask>{formatAddress(address)}</span>
                <span>{amountLabel}</span>
            </div>
            <div className="flex w-full flex-row gap-xxs">
                {hasVesting && <Badge type={BadgeType.Warning} label="Vesting" />}
                {hasMigration && <Badge type={BadgeType.Warning} label="Migration" />}
                {hasAssets && <Badge type={BadgeType.Neutral} label="Assets" />}
            </div>
        </div>
    );
}
