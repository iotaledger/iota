// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { findForbiddenWords } from './findForbiddenWords';

export function censorName(name: string, forbiddenWords: string[]): string {
    const matches = findForbiddenWords(name, forbiddenWords);

    if (matches.length === 0) {
        return name;
    }

    const chars = name.split('');

    for (const match of matches) {
        for (let i = match.start; i < match.end; i++) {
            chars[i] = '*';
        }
    }

    return chars.join('');
}
