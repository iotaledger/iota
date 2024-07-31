// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ChevronDown12, ChevronRight12 } from '@iota/icons';
import * as CollapsiblePrimitive from '@radix-ui/react-collapsible';
import cn from 'clsx';
import { useState, type ReactNode } from 'react';

interface CollapsibleProps {
    title: string;
    defaultOpen?: boolean;
    children: ReactNode | ReactNode[];
    shade?: 'lighter' | 'darker';
    isOpen?: boolean;
    onOpenChange?: (isOpen: boolean) => void;
}

export function Collapsible({
    title,
    children,
    defaultOpen,
    isOpen,
    onOpenChange,
    shade = 'lighter',
}: CollapsibleProps) {
    const [open, setOpen] = useState(isOpen ?? defaultOpen ?? false);

    const handleOpenChange = (isOpen: boolean) => {
        setOpen(isOpen);
        onOpenChange?.(isOpen);
    };

    return (
        <CollapsiblePrimitive.Root
            className="flex w-full flex-shrink-0 flex-col justify-start gap-3"
            open={isOpen ?? open}
            onOpenChange={handleOpenChange}
        >
            <CollapsiblePrimitive.Trigger className="p-0 group flex w-full cursor-pointer items-center gap-2 border-none bg-transparent">
                <div
                    className={cn(
                        'group-hover:text-hero text-captionSmall font-semibold uppercase',
                        {
                            'text-steel': shade === 'lighter',
                            'text-steel-darker': shade === 'darker',
                        },
                    )}
                >
                    {title}
                </div>
                <div
                    className={cn('group-hover:bg-hero h-px flex-1', {
                        'bg-steel': shade === 'darker',
                        'bg-gray-45 group-hover:bg-steel': shade === 'lighter',
                    })}
                />
                <div
                    className={cn('group-hover:text-hero inline-flex', {
                        'text-steel': shade === 'darker',
                        'text-gray-45': shade === 'lighter',
                    })}
                >
                    {open ? <ChevronDown12 /> : <ChevronRight12 />}
                </div>
            </CollapsiblePrimitive.Trigger>

            <CollapsiblePrimitive.Content>{children}</CollapsiblePrimitive.Content>
        </CollapsiblePrimitive.Root>
    );
}
