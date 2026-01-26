'use client';
// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Warning } from '@iota/apps-ui-icons';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import {
    getBrowserCompatibilityMessage,
    LegacyBrowserBanner,
} from '@iota/core/utils/isLegacyBrowser';

import type { ReactNode } from 'react';
import type { FallbackProps } from 'react-error-boundary';
import { ErrorBoundary as ReactErrorBoundary } from 'react-error-boundary';

function Fallback({ error }: FallbackProps): JSX.Element {
    const isCompatibilityError =
        error.message?.includes('structuredClone') || error.name === 'TypeError';

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
