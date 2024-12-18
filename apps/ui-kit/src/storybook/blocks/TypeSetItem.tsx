// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useEffect, useRef, useState } from 'react';

enum FontStyleProperties {
    FontSize = 'Font Size',
    LetterSpacing = 'Letter Spacing',
    FontWeight = 'Font Weight',
    LineHeight = 'Line Height',
    FontFamily = 'Font Family',
}

interface TypeSetItemProps {
    sampleText: string;
    fontClass?: string;
    sizeText?: string;
}

type FontStyles = Record<FontStyleProperties, string>;

function getComputedFontStyles(element: HTMLDivElement): FontStyles {
    const computedStyles = getComputedStyle(element);
    return {
        [FontStyleProperties.FontSize]: computedStyles.fontSize,
        [FontStyleProperties.LetterSpacing]: computedStyles.letterSpacing,
        [FontStyleProperties.FontWeight]: computedStyles.fontWeight,
        [FontStyleProperties.LineHeight]: computedStyles.lineHeight,
        [FontStyleProperties.FontFamily]: computedStyles.fontFamily,
    };
}

export function TypeSetItem({ sampleText, sizeText, fontClass }: TypeSetItemProps) {
    const [styles, setStyles] = useState<FontStyles | undefined>();
    const ref = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        if (ref.current) {
            const computedStyles = getComputedFontStyles(ref.current);
            setStyles(computedStyles);
        }
    }, [ref.current]);

    return (
        <div className="flex flex-row items-start gap-x-md">
            {styles && (
                <div className="text-xs text-neutral-60">
                    {styles[FontStyleProperties.FontSize]}
                </div>
            )}
            <div className="flex flex-col gap-y-sm">
                <span ref={ref} className={fontClass}>
                    {sampleText} {sizeText}
                </span>
                {styles && (
                    <div className="flex flex-row gap-x-xs">
                        {Object.entries(styles)
                            .filter(([key]) => key !== FontStyleProperties.FontSize)
                            .map(([key, value], index) => (
                                <Fragment key={index}>
                                    {index > 0 && (
                                        <span className="text-xs text-neutral-60">|</span>
                                    )}
                                    <span className="text-xs text-neutral-60">
                                        {key}: {value}
                                    </span>
                                </Fragment>
                            ))}
                    </div>
                )}
            </div>
        </div>
    );
}
