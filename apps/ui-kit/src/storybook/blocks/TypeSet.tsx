// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeSetItem } from './TypeSetItem';
import { getFontSizeLabelFromClass } from '../helpers';
import type { TailwindCustomFonts } from '@/lib/tailwind/constants';

export interface TypeSetProps {
    typeset: TailwindCustomFonts;
    label: string;
}

export function TypeSet({ typeset, label }: TypeSetProps) {
    const textClasses = Object.entries(typeset);
    return (
        <div className="mt-4 flex flex-col gap-y-2 rounded-md border border-gray-200 px-xl py-lg shadow-md">
            {textClasses.map(([fontClass, [_, fontConfig]], index) => {
                const sizeText = getFontSizeLabelFromClass(fontClass);
                return (
                    <>
                        <TypeSetItem
                            key={index}
                            sampleText={label}
                            fontClass={fontClass}
                            sizeText={sizeText}
                            {...fontConfig}
                        />
                        {index !== textClasses.length - 1 && <hr className="mb-sm mt-xxs" />}
                    </>
                );
            })}
        </div>
    );
}
