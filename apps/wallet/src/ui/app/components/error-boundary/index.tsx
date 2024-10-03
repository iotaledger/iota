// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Alert, AlertStyle, AlertType } from '_components';
import type { ReactNode } from 'react';
import { ErrorBoundary as ReactErrorBoundary } from 'react-error-boundary';
import type { FallbackProps } from 'react-error-boundary';
import { useLocation } from 'react-router-dom';

function Fallback({ error }: FallbackProps) {
    return (
        <div className="flex h-full w-full items-center justify-center p-2">
            <Alert
                title="Something went wrong"
                supportingText={error?.message ?? 'An error occurred'}
                style={AlertStyle.Default}
                type={AlertType.Warning}
            />
        </div>
    );
}

export interface ErrorBoundaryProps {
    children: ReactNode | ReactNode[];
}

export function ErrorBoundary({ children }: ErrorBoundaryProps) {
    const location = useLocation();
    return (
        <ReactErrorBoundary FallbackComponent={Fallback} resetKeys={[location]}>
            {children}
        </ReactErrorBoundary>
    );
}
