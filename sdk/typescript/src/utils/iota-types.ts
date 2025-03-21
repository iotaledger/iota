// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromB58, splitGenericParameters } from '@iota/bcs';

const TX_DIGEST_LENGTH = 32;

/** Returns whether the tx digest is valid based on the serialization format */
export function isValidTransactionDigest(value: string): value is string {
    try {
        const buffer = fromB58(value);
        return buffer.length === TX_DIGEST_LENGTH;
    } catch (e) {
        return false;
    }
}

// TODO - can we automatically sync this with rust length definition?
// Source of truth is
// https://github.com/iotaledger/iota/blob/acb2b97ae21f47600e05b0d28127d88d0725561d/crates/iota-types/src/base_types.rs#L67
// which uses the Move account address length
// https://github.com/move-language/move/blob/67ec40dc50c66c34fd73512fcc412f3b68d67235/language/move-core/types/src/account_address.rs#L23 .

export const IOTA_ADDRESS_LENGTH = 32;
export function isValidIotaAddress(value: string): value is string {
    return isHex(value) && getHexByteLength(value) === IOTA_ADDRESS_LENGTH;
}

export function isValidIotaObjectId(value: string): boolean {
    return isValidIotaAddress(value);
}

type StructTag = {
    address: string;
    module: string;
    name: string;
    typeParams: (string | StructTag)[];
};

function parseTypeTag(type: string): string | StructTag {
    if (!type.includes('::')) return type;

    return parseStructTag(type);
}

export function parseStructTag(type: string): StructTag {
    const [address, module] = type.split('::');

    const rest = type.slice(address.length + module.length + 4);
    const name = rest.includes('<') ? rest.slice(0, rest.indexOf('<')) : rest;
    const typeParams = rest.includes('<')
        ? splitGenericParameters(rest.slice(rest.indexOf('<') + 1, rest.lastIndexOf('>'))).map(
              (typeParam) => parseTypeTag(typeParam.trim()),
          )
        : [];

    return {
        address: normalizeIotaAddress(address),
        module,
        name,
        typeParams,
    };
}

export function normalizeStructTag(type: string | StructTag): string {
    const { address, module, name, typeParams } =
        typeof type === 'string' ? parseStructTag(type) : type;

    const formattedTypeParams =
        typeParams?.length > 0
            ? `<${typeParams
                  .map((typeParam) =>
                      typeof typeParam === 'string' ? typeParam : normalizeStructTag(typeParam),
                  )
                  .join(',')}>`
            : '';

    return `${address}::${module}::${name}${formattedTypeParams}`;
}

/**
 * Normalize an IOTA address to ensure consistent format.
 * Perform the following operations:
 * 1. Make the address lower case
 * 2. Prepend `0x` if the string does not start with `0x`.
 * 3. Add more zeros if the length of the address(excluding `0x`) is less than `IOTA_ADDRESS_LENGTH`
 *
 * WARNING: if the address value itself starts with `0x`, e.g., `0x0x`, the default behavior
 * is to treat the first `0x` not as part of the address. The default behavior can be overridden by
 * setting `forceAdd0x` to true
 *
 * @param value The address to normalize
 * @param forceAdd0x Whether to add 0x prefix without removing any existing 0x prefixes
 * @param validate Whether to validate the return address
 * @returns The normalized address
 * @throws Error if flag `validate` enabled and the address contains invalid hex characters
 */
export function normalizeIotaAddress(
    value: string,
    forceAdd0x: boolean = false,
    validate: boolean = false,
): string {
    let address = value.toLowerCase().replace(/ /g, '');
    if (!forceAdd0x && address.startsWith('0x')) {
        address = address.slice(2);
    }
    address = `0x${address.padStart(IOTA_ADDRESS_LENGTH * 2, '0')}`;
    if (validate && !isValidIotaAddress(address)) {
        throw new Error(`Invalid IOTA address: ${value}`);
    } else {
        return address;
    }
}

export function normalizeIotaObjectId(
    value: string,
    forceAdd0x: boolean = false,
    validate: boolean = false,
): string {
    return normalizeIotaAddress(value, forceAdd0x, validate);
}

function isHex(value: string): boolean {
    return /^(0x|0X)?[a-fA-F0-9]+$/.test(value) && value.length % 2 === 0;
}

function getHexByteLength(value: string): number {
    return /^(0x|0X)/.test(value) ? (value.length - 2) / 2 : value.length / 2;
}
