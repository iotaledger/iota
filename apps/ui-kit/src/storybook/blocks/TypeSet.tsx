// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeSetItem } from './TypeSetItem';
import { getFontSizeLabelFromClass } from '../helpers';
import { TailwindCustomFonts } from '@/lib/tailwind/constants';

export interface TypeSetProps {
    typeset: TailwindCustomFonts;
    fontWeight: number;
    fontFamily: string;
    label: string;
}

export function TypeSet({ typeset, label, fontWeight, fontFamily }: TypeSetProps) {
    return (
        <div>
            <p>Font Weight: {fontWeight}</p>
            <span>Font Family: {fontFamily}</span>

            <div className="mt-4 flex flex-col gap-y-2 rounded-md border border-gray-200 px-xl py-lg shadow-md">
                {Object.entries(typeset).map(([fontClass, [fontSize]], index) => {
                    const size = Number(fontSize.replace('px', ''));
                    const sizeText = getFontSizeLabelFromClass(fontClass);
                    return (
                        <TypeSetItem
                            key={index}
                            sampleText={label}
                            fontClass={fontClass}
                            fontSize={size}
                            sizeText={sizeText}
                        />
                    );
                })}
            </div>
        </div>
    );
}
