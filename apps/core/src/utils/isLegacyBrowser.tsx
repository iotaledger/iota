'use client';

// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from 'react';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import { MIN_BROWSER_VERSION } from '../constants/minBrowserVersion';

export function getBrowserCompatibilityMessage(): string | null {
    if (typeof navigator === 'undefined') return null;

    try {
        const ua = navigator.userAgent;
        const version = (re: RegExp) => Number(ua.match(re)?.[1] || 999);

        const isLegacy =
            version(/Chrome\/(\d+)/) < MIN_BROWSER_VERSION.chrome ||
            version(/Firefox\/(\d+)/) < MIN_BROWSER_VERSION.firefox ||
            (/Safari/.test(ua) &&
                !/Chrome/.test(ua) &&
                parseFloat(ua.match(/Version\/(\d+\.\d+)/)?.[1] || '99') <
                    MIN_BROWSER_VERSION.safari) ||
            version(/Edg\/(\d+)/) < MIN_BROWSER_VERSION.edge ||
            version(/OPR\/(\d+)/) < MIN_BROWSER_VERSION.opera;

        return isLegacy
            ? 'Your browser version is outdated and may not be compatible. Please update it to the latest version.'
            : null;
    } catch {
        return 'Could not detect browser compatibility. Please update your browser.';
    }
}

export function LegacyBrowserBanner() {
    const [message, setMessage] = useState<string | null>(null);

    useEffect(() => {
        const msg = getBrowserCompatibilityMessage();
        if (msg) setMessage(msg);
    }, []);

    if (!message) return null;

    return (
        <div className="fixed right-0 left-0 top-0 z-[9999] w-full">
            <InfoBox
                title="Compatibility Warning"
                supportingText={message}
                icon={<Warning />}
                type={InfoBoxType.Warning}
                style={InfoBoxStyle.Elevated}
            />
        </div>
    );
}
