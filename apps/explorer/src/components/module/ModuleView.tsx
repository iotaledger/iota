// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaMoveNormalizedType } from '@iota/iota-sdk/client';
import { Prism } from 'prism-react-renderer';
import 'prism-themes/themes/prism-one-light.css';
import { useMemo } from 'react';

import { useNormalizedMoveModule } from '~/hooks/useNormalizedMoveModule';

import { SyntaxHighlighter } from '../syntax-highlighter';

// Include Rust language support.
// TODO: Write a custom prismjs syntax for Move Bytecode.
globalThis.Prism = Prism;
// @ts-expect-error: This file is untyped:
import('prismjs/components/prism-rust').catch(() => {});

interface ModuleViewProps {
    id?: string;
    name: string;
    code: string;
}

interface TypeReference {
    address: string;
    module: string;
    name: string;
    typeArguments: IotaMoveNormalizedType[];
}

/** Takes a normalized move type and returns the address information contained within it */
function unwrapTypeReference(type: IotaMoveNormalizedType): null | TypeReference {
    if (typeof type === 'object') {
        if ('Struct' in type) {
            return type.Struct;
        }
        if ('Reference' in type) {
            return unwrapTypeReference(type.Reference);
        }
        if ('MutableReference' in type) {
            return unwrapTypeReference(type.MutableReference);
        }
        if ('Vector' in type) {
            return unwrapTypeReference(type.Vector);
        }
    }
    return null;
}

export function ModuleView({ id, name, code }: ModuleViewProps): JSX.Element {
    const { data: normalizedModule } = useNormalizedMoveModule(id, name);
    const normalizedModuleReferences = useMemo(() => {
        const typeReferences: Record<string, TypeReference> = {};
        if (!normalizedModule) {
            return typeReferences;
        }
        Object.values(normalizedModule.exposedFunctions).forEach((exposedFunction) => {
            exposedFunction.parameters.forEach((param) => {
                const unwrappedType = unwrapTypeReference(param);
                if (!unwrappedType) return;
                typeReferences[unwrappedType.name] = unwrappedType;

                unwrappedType.typeArguments.forEach((typeArg) => {
                    const unwrappedTypeArg = unwrapTypeReference(typeArg);
                    if (!unwrappedTypeArg) return;
                    typeReferences[unwrappedTypeArg.name] = unwrappedTypeArg;
                });
            });
        });
        return typeReferences;
    }, [normalizedModule]);

    return (
        <SyntaxHighlighter
            code={code}
            language="rust"
            normalizedModuleReferences={normalizedModuleReferences}
            id={id}
        />
    );
}
