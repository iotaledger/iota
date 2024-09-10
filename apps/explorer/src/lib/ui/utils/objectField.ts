// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaMoveNormalizedType } from '@iota/iota-sdk/client';

type TypeReference =
    | {
          address: string;
          module: string;
          name: string;
          typeArguments?: IotaMoveNormalizedType[];
      }
    | string
    | number;

// Get content inside <> and split by , to get underlying object types
function getContentInsideBrackets(input: string): string {
    return input?.slice(input?.indexOf('<') + 1, input?.lastIndexOf('>'));
}

function splitByCommaExcludingBrackets(input: string): string[] {
    const regex = /,(?![^<>]*>)/;
    return input.split(regex).map((part) => part.trim());
}

export function extractSerializationType(type: IotaMoveNormalizedType | ''): TypeReference {
    if (typeof type === 'string') {
        return type;
    }

    if ('TypeParameter' in type) {
        return type.TypeParameter;
    }

    if ('Reference' in type) {
        return extractSerializationType(type.Reference);
    }

    if ('MutableReference' in type) {
        return extractSerializationType(type.MutableReference);
    }

    if ('Vector' in type) {
        return extractSerializationType(type.Vector);
    }

    if ('Struct' in type) {
        return type.Struct;
    }

    return type;
}

function getDisplayName(type: IotaMoveNormalizedType | '', objectType: string): string {
    const normalizedType = extractSerializationType(type);

    if (typeof normalizedType === 'string') {
        let parentKey = null;
        if (typeof type === 'object') {
            if ('Vector' in type) {
                parentKey = 'Vector';
            } else if ('Reference' in type) {
                parentKey = 'Reference';
            } else if ('MutableReference' in type) {
                parentKey = 'MutableReference';
            } else if ('TypeParameter' in type) {
                parentKey = 'TypeParameter';
            } else {
                parentKey = '';
            }
        }
        return parentKey ? `${parentKey}<${normalizedType}>` : normalizedType;
    }

    if (typeof normalizedType === 'number') {
        const typeParameter = splitByCommaExcludingBrackets(getContentInsideBrackets(objectType));

        return typeParameter?.[normalizedType]?.split('::').pop() || '';
    }

    const { name } = normalizedType;
    let typeParam = '';

    // For nested Structs type.typeArguments  append the typeArguments to the name
    // Balance<XUS> || Balance<LSP<IOTA, USDT>>
    if (normalizedType.typeArguments?.length) {
        typeParam = `<${normalizedType.typeArguments
            .map((typeArg) => getDisplayName(typeArg, objectType))
            .join(', ')}>`;
    }

    return `${name}${typeParam}`;
}

interface FieldTypeValue {
    displayName: string;
    normalizedType: string | number;
}

export function getFieldTypeValue(
    type: IotaMoveNormalizedType | '',
    objectType: string,
): FieldTypeValue {
    const displayName = getDisplayName(type, objectType);

    const normalizedType = extractSerializationType(type);
    if (typeof normalizedType === 'string' || typeof normalizedType === 'number') {
        return {
            displayName,
            normalizedType: normalizedType,
        };
    }

    const { address, module, name } = normalizedType;

    return {
        displayName,
        normalizedType: `${address}::${module}::${name}`,
    };
}
