// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isValidIotaAddress } from '@iota/iota-sdk/utils';
import { useEffect } from 'react';
import { toast } from './toaster';

export function ClipboardPasteSafetyWrapper({ children }: { children: React.ReactNode }) {
    useEffect(() => {
        const handlePaste = (event: ClipboardEvent) => {
            const clipboardData = event.clipboardData;
            if (!clipboardData) return;

            const pastedText = clipboardData.getData('text/plain').trim();
            if (!pastedText) return;

            if (isValidIotaAddress(pastedText)) {
                toast.warning('Double-check the pasted address to ensure it is correct.');
            }
        };

        window.addEventListener('paste', handlePaste);
        return () => window.removeEventListener('paste', handlePaste);
    }, []);

    return <>{children}</>;
}
