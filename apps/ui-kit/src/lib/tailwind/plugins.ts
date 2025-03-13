// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import plugin from 'tailwindcss/plugin';

export const firefoxPlugin = plugin(({ addVariant }) => {
    addVariant('firefox', '@-moz-document url-prefix()');
});
