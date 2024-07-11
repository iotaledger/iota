// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { ChevronDown12 } from '@iota/icons';
import * as SelectPrimitive from '@radix-ui/react-select';
import { forwardRef } from 'react';

const Select = SelectPrimitive.Root;
const SelectValue = SelectPrimitive.Value;

const SelectTrigger = forwardRef<
    React.ElementRef<typeof SelectPrimitive.Trigger>,
    React.ComponentPropsWithoutRef<typeof SelectPrimitive.Trigger>
>(({ className, children, ...props }, ref) => (
    <SelectPrimitive.Trigger
        ref={ref}
        className="group flex cursor-pointer items-center gap-0.5 rounded-lg border border-solid border-gray-45 bg-white px-4 py-3 text-steel-dark shadow-sm transition hover:border-steel hover:text-steel-darker focus:outline-none active:bg-hero/5 active:text-steel-dark disabled:cursor-default disabled:border-gray-45 disabled:bg-white disabled:text-gray-60"
        {...props}
    >
        {children}
        <SelectPrimitive.Icon asChild>
            <ChevronDown12 className="text-steel transition group-hover:text-steel-darker group-active:text-steel-dark group-disabled:text-gray-45" />
        </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
));
SelectTrigger.displayName = SelectPrimitive.Trigger.displayName;

const SelectContent = forwardRef<
    React.ElementRef<typeof SelectPrimitive.Content>,
    React.ComponentPropsWithoutRef<typeof SelectPrimitive.Content>
>(({ className, children, ...props }, ref) => (
    <SelectPrimitive.Portal>
        <SelectPrimitive.Content
            ref={ref}
            className="z-[99999] min-w-[112px] bg-transparent"
            {...props}
        >
            <SelectPrimitive.Viewport className="rounded-lg border border-solid border-gray-45 bg-white p-2 shadow-sm">
                {children}
            </SelectPrimitive.Viewport>
        </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
));
SelectContent.displayName = SelectPrimitive.Content.displayName;

const SelectItem = forwardRef<
    React.ElementRef<typeof SelectPrimitive.Item>,
    React.ComponentPropsWithoutRef<typeof SelectPrimitive.Item>
>(({ className, children, ...props }, ref) => (
    <SelectPrimitive.Item
        ref={ref}
        className="flex cursor-pointer items-center rounded-md p-2 text-steel-dark outline-none transition hover:bg-hero/5 hover:text-steel-darker"
        {...props}
    >
        <SelectPrimitive.ItemText>
            <Text variant="body" weight="semibold">
                {children}
            </Text>
        </SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
));
SelectItem.displayName = SelectPrimitive.Item.displayName;

export { Select, SelectTrigger, SelectContent, SelectItem, SelectValue };
