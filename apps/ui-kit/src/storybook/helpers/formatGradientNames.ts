// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function formatGradientNames(
    title: string,
    colors: Record<string, string>,
): Record<string, string> {
    const formattedColors: Record<string, string> = {};

    Object.entries(colors).forEach(([key, value]) => {
        formattedColors[key.replace(`${title}-`, '')] = value;
    });

    return formattedColors;
}
