// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useOnScreen } from '@iota/core';
import { useRef, useEffect, useState } from 'react';
import { Highlight } from 'prism-react-renderer';
import 'prism-themes/themes/prism-one-light.css';

import type { Language } from 'prism-react-renderer';

interface SyntaxHighlighterProps {
    code: string;
    language: Language;
}
const MAX_LINES = 500;
// Use scroll to load more lines of code to prevent performance issues with large code blocks
export function SyntaxHighlighter({ code, language }: SyntaxHighlighterProps): JSX.Element {
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
            <Highlight code={code} language={language} theme={undefined}>
                {({ style, tokens, getLineProps, getTokenProps }) => (
                    <pre className="overflow-auto bg-transparent !p-0 font-medium" style={style}>
                        {tokens.slice(0, loadedLines).map((line, i) => (
                            <div {...getLineProps({ line, key: i })} key={i} className="table-row">
                                <div className="table-cell select-none pr-4 text-left opacity-50">
                                    {i + 1}
                                </div>

                                {line.map((token, key) => (
                                    <span
                                        {...getTokenProps({
                                            token,
                                            key,
                                        })}
                                        key={key}
                                    />
                                ))}
                            </div>
                        ))}
                    </pre>
                )}
            </Highlight>
            <div ref={observerElem} />
        </div>
    );
}
