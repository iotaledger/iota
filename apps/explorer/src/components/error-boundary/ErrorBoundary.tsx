// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ErrorBoundary as ReactErrorBoundary } from 'react-error-boundary';
import { useLocation } from 'react-router-dom';
import type { ReactNode } from 'react';
import type { FallbackProps } from 'react-error-boundary';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

function Fallback({ error }: FallbackProps): JSX.Element {
    return (
        <InfoBox
            title="Error"
            supportingText={error.message}
            icon={<Warning />}
            type={InfoBoxType.Error}
            style={InfoBoxStyle.Elevated}
        />
    );
}

type ErrorBoundaryProps = {
    children: ReactNode | ReactNode[];
};

export function ErrorBoundary({ children }: ErrorBoundaryProps): JSX.Element {
    const location = useLocation();
    return (
        <ReactErrorBoundary FallbackComponent={Fallback} resetKeys={[location]}>
            {children}
        </ReactErrorBoundary>
    );
}
