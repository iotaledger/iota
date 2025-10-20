'use client';

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from 'react';
import { Warning } from '@iota/apps-ui-icons';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import type { ReactNode } from 'react';
import type { FallbackProps } from 'react-error-boundary';
import { ErrorBoundary as ReactErrorBoundary } from 'react-error-boundary';

function getBrowserCompatibilityMessage(): string | null {
    try {
        const ua = navigator.userAgent;
        const version = (re: RegExp) => Number(ua.match(re)?.[1] || 999);

        const isLegacy =
            version(/Chrome\/(\d+)/) < 92 ||
            version(/Firefox\/(\d+)/) < 94 ||
            (/Safari/.test(ua) &&
                !/Chrome/.test(ua) &&
                parseFloat(ua.match(/Version\/(\d+\.\d+)/)?.[1] || '99') < 15.4) ||
            version(/Edg\/(\d+)/) < 98 ||
            version(/OPR\/(\d+)/) < 84;

        return isLegacy
            ? 'Your browser version is outdated and may not be compatible. Please update it to the latest version.'
            : null;
    } catch {
        return 'Could not detect browser compatibility. Please update your browser.';
    }
}

function LegacyBrowserBanner() {
    const [message, setMessage] = useState<string | null>(null);

    useEffect(() => {
        const msg = getBrowserCompatibilityMessage();
        if (msg) setMessage(msg);
    }, []);

    if (!message) return null;

    return (
        <div className="fixed right-4 top-4 z-[9999] max-w-sm">
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

function Fallback({ error }: FallbackProps): JSX.Element {
    const isCompatibilityError =
        error.message?.includes('structuredClone') ||
        error.message?.includes('DataCloneError') ||
        error.name === 'TypeError';

    const message = isCompatibilityError
        ? (getBrowserCompatibilityMessage() ??
          'Your browser version is outdated and may not be compatible. Please update it to the latest version.')
        : error.message || 'An unexpected error occurred.';

    return (
        <div className="fixed right-4 top-4 z-[9999] max-w-sm">
            <InfoBox
                title={isCompatibilityError ? 'Compatibility Warning' : 'Application Error'}
                supportingText={message}
                icon={<Warning />}
                type={isCompatibilityError ? InfoBoxType.Warning : InfoBoxType.Error}
                style={InfoBoxStyle.Elevated}
            />
        </div>
    );
}

export function ErrorBoundary({ children }: { children: ReactNode }) {
    return (
        <>
            <LegacyBrowserBanner />
            <ReactErrorBoundary FallbackComponent={Fallback}>{children}</ReactErrorBoundary>
        </>
    );
}

// export function ErrorTest() {
//     useEffect(() => {
//         delete window.structuredClone;

//         structuredClone({});
//     }, []);

//     return (
//         <div style={{ padding: '1rem' }}>
//             <h2>Testing missing structuredClone...</h2>
//             <p>This should trigger the ErrorBoundary fallback.</p>
//         </div>
//     );
// }
