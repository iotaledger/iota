// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const SIZE_TO_LABEL_MAP: Record<string, string> = {
    sm: 'Small',
    md: 'Medium',
    lg: 'Large',
};

export function getFontSizeLabelFromClass(fontClass: string): string {
    const fontClassSplit = fontClass.split('-');
    const size = fontClassSplit[fontClassSplit.length - 1];
    const label = SIZE_TO_LABEL_MAP[size];
    return label;
}
