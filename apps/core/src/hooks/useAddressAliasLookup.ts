// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '../enums';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { useIotaClientQuery } from '@iota/dapp-kit';

const ADDRESSES_ALIAS_FALLBACK: KnownAddressAliasesFeature = {
    enabled: false,
    addresses: {},
};

type AddressAliases = Record<string, string>;

type KnownAddressAliasesFeature = {
    enabled: boolean;
    addresses: AddressAliases;
};

export function useAddressAliasLookup() {
    const knownAddresses = useFeatureValue<KnownAddressAliasesFeature>(
        Feature.KnownAddressAlias,
        ADDRESSES_ALIAS_FALLBACK,
    );

    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');

    const validatorsAddresses = Object.fromEntries(
        systemState?.activeValidators.map((validator) => [validator.iotaAddress, validator.name]) ??
            [],
    );

    const addressAliasMap = {
        ...validatorsAddresses,
        ...knownAddresses.addresses,
    };

    return (address: string): string | null => {
        if (!knownAddresses || !knownAddresses.enabled) {
            return null;
        }

        const normalized = normalizeIotaAddress(address);
        const addressAlias = addressAliasMap[normalized];

        return addressAlias;
    };
}
