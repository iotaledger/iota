// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '@iota/ui';
import * as RadixDropdownMenu from '@radix-ui/react-dropdown-menu';
import { type ReactNode } from 'react';
import { Root, Thumb } from '@radix-ui/react-switch';

type DropdownMenuProps = {
    content: ReactNode;
    trigger: ReactNode;
    side?: 'top' | 'right' | 'bottom' | 'left';
    align?: 'start' | 'center' | 'end';
} & Omit<RadixDropdownMenu.DropdownMenuProps, 'className' | 'asChild'>;

export function DropdownMenu({
    content,
    side,
    trigger,
    align,
    ...radixRootProps
}: DropdownMenuProps): JSX.Element {
    return (
        <RadixDropdownMenu.Root {...radixRootProps}>
            <RadixDropdownMenu.Trigger className="text-steel hover:text-steel-dark data-[state=open]:text-steel-dark">
                {trigger}
            </RadixDropdownMenu.Trigger>
            <RadixDropdownMenu.Portal>
                <RadixDropdownMenu.Content
                    side={side}
                    align={align}
                    className="z-10 min-w-[280px] rounded-md bg-white p-1 shadow-effect-ui-regular"
                >
                    {content}
                </RadixDropdownMenu.Content>
            </RadixDropdownMenu.Portal>
        </RadixDropdownMenu.Root>
    );
}

type DropdownMenuCheckboxItemProps = Omit<
    RadixDropdownMenu.DropdownMenuCheckboxItemProps,
    'className' | 'checked' | 'asChild'
> & { checked?: boolean; label: ReactNode };
export function DropdownMenuCheckboxItem({
    checked = false,
    ...radixRootProps
}: DropdownMenuCheckboxItemProps): JSX.Element {
    const handleCheckedChange = (checked: boolean) => {
        if (radixRootProps.onCheckedChange) {
            radixRootProps.onCheckedChange(checked);
        }
    };
    return (
        <RadixDropdownMenu.CheckboxItem {...radixRootProps} asChild>
            <div className="flex cursor-pointer select-none items-center gap-4 rounded-md p-2 text-steel-dark outline-none transition-colors data-[highlighted]:bg-iota-light/50 data-[highlighted]:text-steel-darker">
                <div className="flex-1">
                    <Text variant="body/medium">Show System Transactions</Text>
                </div>
                <Root
                    className="relative h-3.75 w-[26px] rounded-full bg-gray-60/70 transition-colors data-[state=checked]:bg-success"
                    checked={checked}
                    onCheckedChange={handleCheckedChange}
                >
                    <Thumb className="block h-[11px] w-[11px] translate-x-0.5 rounded-full bg-white transition-transform will-change-transform data-[state=checked]:translate-x-[13px]" />
                </Root>
            </div>
        </RadixDropdownMenu.CheckboxItem>
    );
}
