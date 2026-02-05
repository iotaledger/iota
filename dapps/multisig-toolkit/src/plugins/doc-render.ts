// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import fs from 'fs';

export default function docRender() {
    const rm = fs.readFileSync('./README.md', 'utf-8');

    return {
        content: rm,
    };
}
