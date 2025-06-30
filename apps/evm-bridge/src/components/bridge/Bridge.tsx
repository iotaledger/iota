import { DepositLayer1, DepositLayer2 } from '..';
import { Header } from '@iota/apps-ui-kit';
import { FormProvider, useForm } from 'react-hook-form';
import { useMemo } from 'react';
import { createBridgeFormSchema } from '../../lib/schema/bridgeForm.schema';
import { zodResolver } from '@hookform/resolvers/zod';
import { Feature, useGetAllBalances, useSortedCoinsByCategories } from '@iota/core';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useAllCoinsMetadata } from '../../hooks/useAllCoinsMetadata';
import { BridgeFormInputName } from '../../lib/enums';
import { useAvailableIotaBalanceL1 } from '../../hooks/useAvailableIotaBalanceL1';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useGetAllBalancesL2 } from '../../hooks/useGetAllBalancesL2';
import { useAvailableIotaBalanceL2 } from '../../hooks/useAvailableIotaBalanceL2';

export function Bridge() {
    const address = useCurrentAccount()?.address as string;
    const knownEvmCoins = useFeatureValue(Feature.KnownIotaEVMCoinTypes, []);

    const { data: coinsBalanceL1 } = useGetAllBalances(address);

    const { recognized: recognizedL1, pinned: pinnedL1 } = useSortedCoinsByCategories(
        coinsBalanceL1 || [],
        knownEvmCoins,
    );
    const sortedCoinsBalanceL1 = [...recognizedL1, ...pinnedL1];

    const { metadata: coinsMetadataL1 } = useAllCoinsMetadata(sortedCoinsBalanceL1);
    const { availableBalance: availableIotaBalanceL1 } = useAvailableIotaBalanceL1();

    // Fetch L2 balance for L1 address
    const { data: l1AddressCoinsBalanceInL2 } = useGetAllBalancesL2(address);
    const { recognized: recognizedL2, pinned: pinnedL2 } = useSortedCoinsByCategories(
        l1AddressCoinsBalanceInL2 || [],
        knownEvmCoins,
    );
    const sortedCoinsBalanceL2 = [...recognizedL2, ...pinnedL2];
    const { metadata: coinsMetadataL2 } = useAllCoinsMetadata(sortedCoinsBalanceL2);

    const { availableBalance: availableIotaBalanceL2 } = useAvailableIotaBalanceL2();

    // adjust L1 iota total Balance in sortedCoinsBalanceL1 to available balance
    const updatedSortedCoinsBalanceL1 = sortedCoinsBalanceL1.map((coin) => {
        if (coin.coinType === IOTA_TYPE_ARG) {
            return {
                ...coin,
                totalBalance: availableIotaBalanceL1
                    ? availableIotaBalanceL1.toString()
                    : coin.totalBalance,
            };
        }
        return coin;
    });

    // adjust L2 iota total Balance in sortedCoinsBalanceL2 to available balance
    const updatedSortedCoinsBalanceL2 = sortedCoinsBalanceL2.map((coin) => {
        if (coin.coinType === IOTA_TYPE_ARG) {
            return {
                ...coin,
                totalBalance: availableIotaBalanceL2
                    ? availableIotaBalanceL2.toString()
                    : coin.totalBalance,
            };
        }
        return coin;
    });

    const formSchema = useMemo(
        () =>
            createBridgeFormSchema(
                updatedSortedCoinsBalanceL1,
                updatedSortedCoinsBalanceL2,
                coinsMetadataL1,
                coinsMetadataL2,
            ),
        [updatedSortedCoinsBalanceL1, sortedCoinsBalanceL1, coinsMetadataL1, coinsMetadataL1],
    );

    const formMethods = useForm({
        mode: 'all',
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        resolver: zodResolver(formSchema as any),
        defaultValues: {
            [BridgeFormInputName.IsFromLayer1]: true,
        },
    });
    const isFromLayer1 = formMethods.watch('isFromLayer1');

    return (
        <FormProvider {...formMethods}>
            <div className="relative h-full">
                <BackgroundArrows />

                <div className="rounded-3xl bg-shader-primary-light-8 border-shader-inverted-dark-16 dark:bg-shader-inverted-dark-16 dark:border-shader-primary-light-8 h-full relative backdrop-blur-xl border">
                    <div className="[&_>div]:bg-transparent dark:[&_>div]:bg-transparent">
                        <Header title="Send" />
                    </div>

                    <div className="p-md--rs">
                        {!!isFromLayer1 && <DepositLayer1 />}
                        {!isFromLayer1 && <DepositLayer2 />}
                    </div>
                </div>
            </div>
        </FormProvider>
    );
}

function BackgroundArrows() {
    return (
        <>
            <img
                src="/background-arrow.svg"
                alt="background arrow asset"
                className="absolute top-6 right-0 translate-x-[65%] z-0 pointer-events-none select-none"
            />
            <img
                src="/background-arrow.svg"
                alt="background arrow asset"
                className="absolute rotate-180 bottom-6 left-0 -translate-x-[65%] pointer-events-none select-none"
            />
        </>
    );
}
