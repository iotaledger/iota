'use client';

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
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
            version(/Chrome\/(\d+)/) < 200 ||
            version(/Firefox\/(\d+)/) < 94 ||
            (/Safari/.test(ua) &&
                !/Chrome/.test(ua) &&
                parseFloat(ua.match(/Version\/(\d+\.\d+)/)?.[1] || '99') < 15.4) ||
            version(/Edg\/(\d+)/) < 98 ||
            version(/OPR\/(\d+)/) < 84 ||
            typeof structuredClone !== 'function';

        return isLegacy
            ? 'Your browser version is outdated and may not be compatible. Please update it to the latest version.'
            : null;
    } catch {
        return 'Could not detect browser compatibility. Please update your browser.';
    }
}

function Fallback({ error }: FallbackProps): JSX.Element {
    const compatibilityMessage = getBrowserCompatibilityMessage();
    const isStructuredCloneError = /structuredClone/i.test(error.message);

    let message = error.message;

    if (compatibilityMessage) {
        message = compatibilityMessage;
    } else if (isStructuredCloneError) {
        message =
            'Your browser does not fully support structuredClone. Please update your browser.';
    }

    return (
        <div className="p-4">
            <InfoBox
                title="Error"
                supportingText={message}
                icon={<Warning />}
                type={InfoBoxType.Error}
                style={InfoBoxStyle.Elevated}
            />
        </div>
    );
}

export function ErrorBoundary({ children }: { children: ReactNode }) {
    return <ReactErrorBoundary FallbackComponent={Fallback}>{children}</ReactErrorBoundary>;
}
