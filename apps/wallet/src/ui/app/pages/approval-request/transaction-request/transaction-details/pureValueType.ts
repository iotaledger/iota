// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const PURE_VALUETYPE_LABEL_MAP = {
    address: 'Address',
    signer: 'Signer',
    'vector<u8>': 'String',
    '0x1::string::String': 'String',
    '0x1::ascii::String': 'String',
};

const VECTOR_REGEX = /^vector<(.+)>$/;

function isKnownPureValueType(
    valueType: string,
): valueType is keyof typeof PURE_VALUETYPE_LABEL_MAP {
    return valueType in PURE_VALUETYPE_LABEL_MAP;
}

export function getPureValueTypeLabel(valueType: string | null | undefined): string {
    if (!valueType) return '';

    if (isKnownPureValueType(valueType)) {
        return PURE_VALUETYPE_LABEL_MAP[valueType];
    }

    const isVector = valueType.match(VECTOR_REGEX);
    if (isVector) {
        // vector<address> → Address[]
        return `${getPureValueTypeLabel(isVector[1])}[]`;
    }

    return valueType;
}

export function formatPureInputValue(value: unknown, valueType: string | null | undefined): string {
    if (
        valueType === 'vector<u8>' ||
        valueType === '0x1::string::String' ||
        valueType === '0x1::ascii::String'
    ) {
        try {
            const arr: number[] = Array.isArray(value)
                ? (value as number[])
                : JSON.parse(`[${String(value)}]`);
            return new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(arr));
        } catch {
            // fall through
        }
    }

    if (Array.isArray(value)) {
        return JSON.stringify(value);
    }

    const stringified = String(value);
    const isNumeric = /^\d+$/.test(stringified);
    if (isNumeric) {
        try {
            return BigInt(stringified).toLocaleString();
        } catch {
            // fall through
        }
    }

    return stringified;
}
