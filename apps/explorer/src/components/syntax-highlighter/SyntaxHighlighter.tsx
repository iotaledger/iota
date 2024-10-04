// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useOnScreen } from '@iota/core';
import { useRef, useEffect, useState } from 'react';
import { Highlight, themes } from 'prism-react-renderer';
import type { Language } from 'prism-react-renderer';
import type { IotaMoveNormalizedType } from '@iota/iota-sdk/client';
import { LinkWithQuery } from '../ui';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

interface TypeReference {
    address: string;
    module: string;
    name: string;
    typeArguments: IotaMoveNormalizedType[];
}

interface SyntaxHighlighterBaseProps {
    code: string;
    language: Language;
    normalizedModuleReferences?: Record<string, TypeReference>;
    id?: string;
}

const MAX_LINES = 500;
// Use scroll to load more lines of code to prevent performance issues with large code blocks
export function SyntaxHighlighter({
    code,
    language,
    normalizedModuleReferences,
    id,
}: SyntaxHighlighterBaseProps): JSX.Element {
    const observerElem = useRef<HTMLDivElement | null>(null);
    const { isIntersecting } = useOnScreen(observerElem);
    const [loadedLines, setLoadedLines] = useState(MAX_LINES);
    useEffect(() => {
        if (isIntersecting) {
            setLoadedLines((prev) => prev + MAX_LINES);
        }
    }, [isIntersecting]);
    return (
        <div className="overflow-auto whitespace-pre font-mono text-sm">
            <Highlight code={code} language={language} theme={themes.github}>
                {({ style, tokens, getLineProps, getTokenProps }) => (
                    <pre className="overflow-auto bg-transparent p-xs font-medium" style={style}>
                        {tokens.slice(0, loadedLines).map((line, i) => (
                            <div {...getLineProps({ line, key: i })} key={i} className="table-row">
                                <div className="table-cell select-none pr-4 text-left text-primary-30 opacity-50">
                                    {i + 1}
                                </div>

                                {line.map((token, key) => {
                                    if (normalizedModuleReferences) {
                                        const reference =
                                            normalizedModuleReferences?.[token.content];

                                        if (
                                            (token.types.includes('class-name') ||
                                                token.types.includes('constant')) &&
                                            reference
                                        ) {
                                            const href = `/object/${reference.address}?module=${reference.module}`;
                                            const { key: _, ...tokenProps } = getTokenProps({
                                                token,
                                                key,
                                            });

                                            return (
                                                <LinkWithQuery
                                                    key={key}
                                                    {...tokenProps}
                                                    to={href}
                                                    target={
                                                        normalizeIotaAddress(reference.address) ===
                                                        normalizeIotaAddress(id!)
                                                            ? undefined
                                                            : '_blank'
                                                    }
                                                />
                                            );
                                        }
                                    }

                                    return (
                                        <span
                                            {...getTokenProps({
                                                token,
                                                key,
                                            })}
                                            key={key}
                                        />
                                    );
                                })}
                            </div>
                        ))}
                    </pre>
                )}
            </Highlight>
            <div ref={observerElem} />
        </div>
    );
}
