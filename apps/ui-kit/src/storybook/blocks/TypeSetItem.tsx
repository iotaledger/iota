// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface TypeSetItemProps {
    sampleText: string;
    fontClass: string;
    fontSize: number;
}

export function TypeSetItem({ sampleText, fontClass, fontSize }: TypeSetItemProps) {
    return (
        <div className="flex flex-row gap-x-4">
            <div className="text-xs">{fontSize}</div>
            <div className={fontClass}>{sampleText}</div>
        </div>
    );
}
