// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Transaction } from '@iota/iota-sdk/transactions';

import { normalizeIotaName } from './utils.js';

export function isSubname(name: string): boolean {
    return normalizeIotaName(name, 'dot').split('.').length > 2;
}

/**
 * Checks if a name is a nested subname.
 * A nested subname is a subname that is a subname of another subname.
 * @param name The name to check (e.g test.example.sub.iota)
 */
export function isNestedSubname(name: string): boolean {
    return normalizeIotaName(name, 'dot').split('.').length > 3;
}

/**
 * The years must be between 1 and 5.
 */
export function validateYears(years: number) {
    if (!(years > 0 && years < 6)) throw new Error('Years must be between 1 and 5');
}

export function zeroCoin(tx: Transaction, type: string) {
    return tx.moveCall({
        target: '0x2::coin::zero',
        typeArguments: [type],
    });
}

export function getConfigType(iotaNamesPackageV1: string, innerType: string): string {
    return `${iotaNamesPackageV1}::iota_names::ConfigKey<${innerType}>`;
}

export function getRegistryKeyType(iotaNamesPackageId: string, innerType: string): string {
    return `${iotaNamesPackageId}::iota_names::RegistryKey<${innerType}>`;
}

export function getNameType(iotaNamesPackageV1: string): string {
    return `${iotaNamesPackageV1}::name::Name`;
}

export function getPricelistConfigType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::pricing_config::PricingConfig`;
}

export function getRenewalPricelistConfigType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::pricing_config::RenewalConfig`;
}

export function getNameRegistrationType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::name_registration::NameRegistration`;
}

export function getSubnameRegistrationType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::subname_registration::SubnameRegistration`;
}

export function getCoreConfigType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::core_config::CoreConfig`;
}

export function getDenyListType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::deny_list::DenyList`;
}

export function getSubnamesConfigType(iotaNamesPackageId: string): string {
    return `${iotaNamesPackageId}::config::SubnameConfig`;
}
