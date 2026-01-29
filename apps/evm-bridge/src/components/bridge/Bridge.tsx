// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DepositLayer1, DepositLayer2 } from '..';
import { Header } from '@iota/apps-ui-kit';
import { FormProvider, useForm } from 'react-hook-form';
import { useMemo } from 'react';
import { createBridgeFormSchema } from '../../lib/schema/bridgeForm.schema';
import { zodResolver } from '@hookform/resolvers/zod';
import { useCoinsMetadata } from '../../hooks/useCoinsMetadata';
import { BridgeFormInputName } from '../../lib/enums';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useSortedCoins } from '../../hooks/useSortedCoins';

export function Bridge() {
    const { sortedCoinsL1, sortedCoinsL2 } = useSortedCoins();

    const { metadata: coinsMetadataL1 } = useCoinsMetadata(sortedCoinsL1);
    const { metadata: coinsMetadataL2 } = useCoinsMetadata(sortedCoinsL2);

    const formSchema = useMemo(
        () =>
            createBridgeFormSchema(sortedCoinsL1, sortedCoinsL2, coinsMetadataL1, coinsMetadataL2),
        [sortedCoinsL1, sortedCoinsL2, coinsMetadataL1, coinsMetadataL2],
    );

    const formMethods = useForm({
        mode: 'all',
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        resolver: zodResolver(formSchema as any),
        defaultValues: {
            [BridgeFormInputName.IsFromLayer1]: true,
            [BridgeFormInputName.CoinType]: IOTA_TYPE_ARG,
        },
    });
    const isFromLayer1 = formMethods.watch(BridgeFormInputName.IsFromLayer1);

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
