// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import type { ReactNode } from 'react';

export interface FieldLabelProps {
    txt: string;
    children: ReactNode | ReactNode[];
}

export default function FieldLabel({ txt, children }: FieldLabelProps) {
    return (
        <label className="mt-7.5 flex flex-col flex-nowrap gap-2.5 first:mt-0">
            <div className="ml-2">
                <Text variant="body" color="steel-darker" weight="semibold">
                    {txt}
                </Text>
            </div>

            {children}
        </label>
    );
}
