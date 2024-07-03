// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TypeSetItem } from './TypeSetItem';
import { getTypeSetSize } from '../constants';
import { CustomFontSize } from '@/lib/tailwind/constants';

interface TypeSetProps {
    typeset: CustomFontSize;
    fontWeight: number;
    fontFamily: string;
    label: string;
}

export function TypeSet({ typeset, label, fontWeight, fontFamily }: TypeSetProps) {
    return (
        <div>
            <p className="!m-0">
                Font Weight: <span className="!text-sm !font-semibold">{fontWeight}</span>
            </p>

            <p className="!m-0">
                Font Family: <span className="!text-sm !font-semibold">{fontFamily}</span>
            </p>

            <div className="border border-gray-400 p-4">
                {Object.entries(typeset).map(([fontClass, [fontSize]], index) => {
                    const sizeText = getTypeSetSize(typeset);
                    const size = Number(fontSize.replace('px', ''));
                    return (
                        <TypeSetItem
                            key={index}
                            sampleText={label + ' ' + sizeText}
                            fontClass={'text-' + fontClass}
                            fontSize={size}
                        />
                    );
                })}
            </div>
        </div>
    );
}
