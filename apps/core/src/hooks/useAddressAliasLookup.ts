// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '../enums';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import { useNetwork } from './useNetwork';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { useMemo } from 'react';

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

    const networkValidatorAliases = useMemo(
        () => validatorAliasesByNetwork[network]?.addresses || ADDRESSES_ALIAS_FALLBACK.addresses,
        [network],
    );

    const addressAliasMap = {
        ...networkValidatorAliases,
        ...knownAddresses.addresses,
    };

    return (address: string) => {
        if (!knownAddresses || !knownAddresses.enabled) {
            return null;
        }

        const normalized = normalizeIotaAddress(address);
        const addressAlias = addressAliasMap[normalized];

        return addressAlias;
    };
}
