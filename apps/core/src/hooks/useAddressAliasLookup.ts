// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '../enums';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import { useNetwork } from './useNetwork';
import { useFeatureValue } from '@growthbook/growthbook-react';

const ADDRESSES_ALIAS_FALLBACK: KnownAddressAliasesFeature = {
    enabled: false,
    addresses: {},
};

type AddressAliases = Record<string, string>;

type KnownAddressAliasesFeature = {
    enabled: boolean;
    addresses: AddressAliases;
};

type ValidatorAddressAliasFeature = {
    [key in Network]?: KnownAddressAliasesFeature;
};

export function useAddressAliasLookup() {
    const networkId = useNetwork();
    const network = getNetwork(networkId).id;

    const knownAddresses = useFeatureValue<KnownAddressAliasesFeature>(
        Feature.KnownAddressAlias,
        ADDRESSES_ALIAS_FALLBACK,
    );
    const validatorAliasesByNetwork = useFeatureValue<ValidatorAddressAliasFeature>(
        Feature.ValidatorAddressAlias,
        {},
    );

    const networkValidatorAliases = validatorAliasesByNetwork[network]?.addresses;
    const addressAliasMap = {
        ...networkValidatorAliases,
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
