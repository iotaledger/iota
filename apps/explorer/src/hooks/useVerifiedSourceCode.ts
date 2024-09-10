// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { useIotaClientContext } from '@iota/dapp-kit';
import { Network } from '@iota/iota-sdk/client';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';

type UseVerifiedSourceCodeArgs = {
    packageId: string;
    moduleName: string;
};

type UseVerifiedSourceCodeResponse = {
    source?: string;
    error?: string;
};

const networksWithSourceCodeVerification: Network[] = [
    Network.Devnet,
    Network.Testnet,
    Network.Mainnet,
];

/**
 * Hook that retrieves the source code for verified modules.
 */
export function useVerifiedSourceCode({
    packageId,
    moduleName,
}: UseVerifiedSourceCodeArgs): UseQueryResult<string | null, Error> {
    const { network } = useIotaClientContext();
    const isEnabled = useFeatureIsOn('module-source-verification');

    return useQuery<string | null, Error>({
        queryKey: ['verified-source-code', packageId, moduleName, network],
        queryFn: async () => {
            const response = await fetch(
                `https://source.iota.org/api?network=${network.toLowerCase()}&address=${packageId}&module=${moduleName}`,
            );
            if (!response.ok) {
                throw new Error(`Encountered unexpected response: ${response.status}`);
            }

            const jsonResponse: UseVerifiedSourceCodeResponse = await response.json();
            return jsonResponse.source || null;
        },
        enabled: isEnabled && networksWithSourceCodeVerification.includes(network as Network),
    });
}
