// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';

export function useOnKeyboardEvent<K extends 'keydown' | 'keyup' | 'keypress'>(
    eventType: K,
    keys: string[],
    handler: (e: KeyboardEvent) => void,
    enabled = true,
) {
    useEffect(() => {
        if (enabled) {
            const listener = (e: KeyboardEvent) => {
                if (keys.includes(e.key)) {
                    handler(e);
                }
            };

            document.addEventListener(eventType, listener);

            return () => {
                document.removeEventListener(eventType, listener);
            };
        }
    }, [eventType, keys, handler, enabled]);
}
