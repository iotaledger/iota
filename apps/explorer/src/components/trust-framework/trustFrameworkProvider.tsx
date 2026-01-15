// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useIotaClient, useIotaClientContext } from '@iota/dapp-kit';
import { type IdentityClientReadOnly } from '@iota/identity-wasm/web';
import { type PropsWithChildren, useEffect, useMemo, useState } from 'react';
import { TrustFrameworkContext, type TrustFrameworkProviderContext } from '~/contexts';
import { createIdentityClientReadOnly } from '~/lib/utils/trust-framework/identity';

export function TrustFrameworkProvider({ children }: PropsWithChildren) {
    const { network } = useIotaClientContext();
    const iotaClient = useIotaClient();
    const [identityClient, setIdentityClient] = useState<IdentityClientReadOnly | null>(null);

    useEffect(() => {
        if (!iotaClient) return;

        const instantiateIdentityClient = async () => {
            const _identityClient = await createIdentityClientReadOnly(iotaClient, network);
            setIdentityClient(_identityClient);
        };
        instantiateIdentityClient();
    }, [iotaClient, network]);

    const ctx = useMemo(
        (): TrustFrameworkProviderContext => ({
            identityClient,
        }),
        [identityClient],
    );

    return <TrustFrameworkContext.Provider value={ctx}>{children}</TrustFrameworkContext.Provider>;
}
