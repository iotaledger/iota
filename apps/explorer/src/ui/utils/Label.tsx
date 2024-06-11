// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';

import type { ComponentProps } from 'react';

interface LabelProps extends Omit<ComponentProps<'label'>, 'ref' | 'className'> {
    label: string;
}

export const Label = forwardRef<HTMLLabelElement, LabelProps>(
    ({ label, children, ...labelProps }, ref) => (
        <label
            ref={ref}
            {...labelProps}
            className="flex flex-col flex-nowrap items-stretch gap-2.5"
        >
            <div className="ml-2.5 text-bodySmall font-medium text-steel-darker">{label}</div>
            {children ? <div className="flex flex-col flex-nowrap">{children}</div> : null}
        </label>
    ),
);
