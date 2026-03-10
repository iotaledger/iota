// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GRACE_PERIOD_MS, isSubname, NameRecord } from '@iota/iota-names-sdk';

import type { RegistrationNft } from '../interfaces/registration.interfaces';
import { formatExpirationDate } from './format/formatExpirationDate';

export function getTargetExpirationDate(renewYears: number): string {
    const today = new Date();
    return formatExpirationDate(new Date(today.setFullYear(today.getFullYear() + renewYears)));
}

export function isNameRecordExpired(nameRecord: NameRecord | RegistrationNft) {
    return nameRecord.expirationDate < new Date();
}

export function isGracePeriodExpired(nameRecord: NameRecord | RegistrationNft) {
    return nameRecord.expirationDate.getTime() + GRACE_PERIOD_MS < Date.now();
}

export function isNameRecordCloseToExpiration(nameRecord: NameRecord | RegistrationNft): boolean {
    const expirationThreshold = nameRecord.expirationDate.getTime() - GRACE_PERIOD_MS;
    return !isNameRecordExpired(nameRecord) && expirationThreshold < Date.now();
}

export function getNamePermissions(nameRecord: NameRecord) {
    const isNameSubname = isSubname(nameRecord.name);
    // Names are allowed always
    let allowChildCreation = !isNameSubname;
    let allowTimeExtension = !isNameSubname;
    // But subnames need their permissions checked
    if ('S_AC' in nameRecord.data) {
        allowChildCreation = nameRecord.data.S_AC === '1';
    }
    if ('S_ATE' in nameRecord.data) {
        allowTimeExtension = nameRecord.data.S_ATE === '1';
    }
    return {
        allowChildCreation,
        allowTimeExtension,
    };
}

/**
 * Get the parent object id of a given subname
 */
export function getParentObjectId(
    ownedNames: RegistrationNft[],
    ownedSubnames: RegistrationNft[],
    name: string,
) {
    const parts = name.split('.');
    const parentName = parts.slice(1).join('.');
    const parentParts = parentName?.split('.').length;

    // 2 parts names are names, the rest are subnames
    const parentNames = parentParts === 2 ? ownedNames : ownedSubnames;

    const parent = parentNames.find(({ name }) => name === parentName);
    return parent?.id || null;
}

/**
 * Get the parent object of a given name or subname
 */
export function getParentObject(
    ownedNames: RegistrationNft[],
    ownedSubnames: RegistrationNft[],
    name: string,
) {
    const parts = name.split('.');
    const parentName = parts.slice(1).join('.');
    const names = isSubname(parentName) ? ownedSubnames : ownedNames;
    const parent = names.find(({ name }) => name === parentName);
    return parent || null;
}

/**
 * Get object id of a given name or subname
 */
export function getNameObject(names: RegistrationNft[], name: string) {
    const nameObject = names.find((domain: { name: string | null }) => domain.name === name);
    return nameObject?.id || null;
}

/**
 * Get the amount of years that this name can be renewed as of now.
 */
export function getNameRenewableYears(maxYears: number, expirationDate: Date): number {
    const inMaxYearsTime = new Date();
    inMaxYearsTime.setFullYear(inMaxYearsTime.getFullYear() + maxYears + 1);

    let years = inMaxYearsTime.getFullYear() - expirationDate.getFullYear();

    // Adjust if the expiration day/month has not been reached yet
    const adjustedExpiration = new Date(expirationDate);
    adjustedExpiration.setFullYear(inMaxYearsTime.getFullYear());

    if (adjustedExpiration > inMaxYearsTime) {
        years--;
    }

    return years;
}
