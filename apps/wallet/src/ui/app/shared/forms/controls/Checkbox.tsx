// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Check12 } from '@iota/icons';
import * as RadixCheckbox from '@radix-ui/react-checkbox';
import { forwardRef } from 'react';
import type { ComponentProps, ReactNode } from 'react';

type CheckboxProps = {
    label: ReactNode;
} & Omit<ComponentProps<typeof RadixCheckbox.Root>, 'className' | 'ref' | 'id'>;

export const Checkbox = forwardRef<HTMLButtonElement, CheckboxProps>(
    ({ label, ...props }, forwardedRef) => (
        <div className="flex items-center gap-2 pl-2.5">
            <RadixCheckbox.Root
                className="group peer m-0 appearance-none border-0 bg-transparent p-0"
                ref={forwardedRef}
                id={props.name}
                {...props}
            >
                <div className="border-steel disabled:border-hero-darkest/10 group-data-[state=checked]:bg-success flex h-5 w-5 items-center justify-center rounded border border-solid bg-white group-data-[state=checked]:border-0">
                    <Check12 className="text-hero-darkest/10 text-body font-semibold group-data-[state=checked]:text-white" />
                </div>
            </RadixCheckbox.Root>
            <label
                className="text-steel-dark peer-disabled:text-gray-60 peer-data-[state=checked]:text-steel-darker text-body font-medium"
                htmlFor={props.name}
            >
                {label}
            </label>
        </div>
    ),
);
