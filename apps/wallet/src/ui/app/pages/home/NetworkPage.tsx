// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NetworkSelector } from '_components';
import { FaucetRequestButton } from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { PageTemplate } from '_src/ui/app/components/PageTemplate';
import { useAppSelector } from '_hooks';
import { getCustomNetwork } from '@iota/core';
import { getNetwork } from '@iota/iota-sdk/client';

export function NetworkPage() {
    const network = useAppSelector(({ app }) => app.network);
    const customRpc = useAppSelector(({ app }) => app.customRpc);
    const networkConfig = customRpc ? getCustomNetwork(customRpc) : getNetwork(network);
    const hasFaucet = !!networkConfig?.faucet;

    return (
        <PageTemplate title="Network">
            <div className="flex h-full flex-col">
                <NetworkSelector />
                {hasFaucet && (
                    <>
                        <div className="flex-1" />
                        <div className="border-t border-shader-neutral-light-8 pt-md dark:border-shader-neutral-dark-8">
                            <p className="mb-sm text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                Get free tokens for testing on this network
                            </p>
                            <FaucetRequestButton />
                        </div>
                    </>
                )}
            </div>
        </PageTemplate>
    );
}
