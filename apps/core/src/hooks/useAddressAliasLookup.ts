// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import { Feature } from '../enums';
import { trimOrFormatAddress } from '@iota/iota-sdk/utils';
import { useCallback } from 'react';

export interface GetAddressAliasParams {
    address: string;
    formatUnknownAddress?: boolean;
}

export function useAddressAliasLookup() {
    const knownAddressesFeature = useFeatureValue<{
        enabled: boolean;
        addresses: Record<string, string>;
    }>(Feature.KnownAddressAlias as string, {
        enabled: false,
        addresses: {},
    });

    return useCallback(
        ({ address, formatUnknownAddress: formatUnknownAddress }: GetAddressAliasParams) => {
            const formattedAddress = trimOrFormatAddress(address);

            if (!knownAddressesFeature.enabled) {
                return {
                    address: formatUnknownAddress ? trimOrFormatAddress(address) : address,
                    alias: undefined,
                };
            }

            const addressAlias = knownAddressesFeature.addresses[formattedAddress];
            const isKnownAddress = !!addressAlias;

            return {
                address: isKnownAddress || formatUnknownAddress ? formattedAddress : address,
                alias: addressAlias,
            };
        },
        [knownAddressesFeature],
    );
}
