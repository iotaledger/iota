// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useIotaClient } from '@iota/dapp-kit';
import { IdentityClientReadOnly } from '@iota/identity-wasm/web';
import { type PropsWithChildren, useEffect, useMemo, useState } from 'react';
import { TrustFrameworkContext, type TrustFrameworkProviderContext } from '~/contexts';
import { IOTA_IDENTITY_PKG_ID } from '~/lib/constants/trustFramework.constants';
import { initIdentityWasmWeb } from '~/lib/utils/trust-framework/identity';

export function TrustFrameworkProvider({ children }: PropsWithChildren) {
    const iotaClient = useIotaClient();
    const [identityClient, setIdentityClient] = useState<IdentityClientReadOnly | null>(null);

    useEffect(() => {
        if (!iotaClient) return;

        const instantiateIdentityClient = async () => {
            await initIdentityWasmWeb();
            const _identityClient = await IdentityClientReadOnly.createWithPkgId(
                iotaClient,
                IOTA_IDENTITY_PKG_ID,
            );
            setIdentityClient(_identityClient);
        };
        instantiateIdentityClient();
    }, [iotaClient]);

    const ctx = useMemo(
        (): TrustFrameworkProviderContext => ({
            identityClient,
        }),
        [identityClient],
    );

    return <TrustFrameworkContext.Provider value={ctx}>{children}</TrustFrameworkContext.Provider>;
}
