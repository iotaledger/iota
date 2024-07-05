// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ThemedOrDefault } from '../types';
import { Theme } from '../enums';

export function resolveThemedClasses(
    themedOrDefaultClasses: ThemedOrDefault,
    theme: Theme,
): string {
    return typeof themedOrDefaultClasses === 'string'
        ? themedOrDefaultClasses
        : themedOrDefaultClasses[theme];
}
