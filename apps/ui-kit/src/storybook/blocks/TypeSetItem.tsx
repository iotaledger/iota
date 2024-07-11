// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface TypeSetItemProps {
    sampleText: string;
    fontClass?: string;
    fontSize?: number;
    sizeText?: string;
}

export function TypeSetItem({ sampleText, fontClass, fontSize, sizeText }: TypeSetItemProps) {
    return (
        <div className="flex flex-row items-center gap-x-4">
            <div className="text-label-md text-gray-500">{fontSize}</div>
            <div className={fontClass}>
                {sampleText} {sizeText}
            </div>
        </div>
    );
}
