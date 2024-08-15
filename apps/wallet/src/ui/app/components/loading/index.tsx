// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';
import LoadingIndicator, { type LoadingIndicatorProps } from './LoadingIndicator';

interface LoadingProps extends LoadingIndicatorProps {
    loading: boolean;
    children: ReactNode | ReactNode[];
}

export function Loading({ loading, children, ...indicatorProps }: LoadingProps) {
    return loading ? (
        <div className="flex h-full items-center justify-center">
            <LoadingIndicator {...indicatorProps} />
        </div>
    ) : (
        <>{children}</>
    );
}

export default Loading;
