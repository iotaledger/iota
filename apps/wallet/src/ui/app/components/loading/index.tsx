// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator, type LoadingIndicatorProps } from '@iota/apps-ui-kit';
import type { ReactNode } from 'react';

interface LoadingProps extends LoadingIndicatorProps {
    loading: boolean;
    children: ReactNode | ReactNode[];
}

export function Loading({ loading, children, ...indicatorProps }: LoadingProps) {
    return loading ? (
        <div className="flex h-full w-full items-center justify-center">
            <LoadingIndicator {...indicatorProps} />
        </div>
    ) : (
        <>{children}</>
    );
}
