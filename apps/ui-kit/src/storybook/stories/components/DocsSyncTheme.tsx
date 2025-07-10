// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { UIKitTheme } from '../../../lib/enums/theme.enums';
import { useEffect } from 'react';

export function DocsSyncTheme() {
    useEffect(() => {
        const docsUrl = new URL(document.location.href);
        const globals = docsUrl.searchParams.get('globals');

        const currentTheme = globals?.replace('theme:', '') || UIKitTheme.Light;

        for (const theme of Object.values(UIKitTheme)) {
            document.documentElement.classList.toggle(theme, theme === currentTheme);
        }
    }, []);

    return null;
}
