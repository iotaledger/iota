// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';

export interface LabelValuesContainerProps {
    children: ReactNode;
}

export function LabelValuesContainer({ children }: LabelValuesContainerProps) {
    return <div className="flex flex-col flex-nowrap gap-3 text-body font-medium">{children}</div>;
}
