// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Accordion, AccordionContent, AccordionHeader, Title, TitleSize } from '@iota/apps-ui-kit';
import { useState, type ReactNode } from 'react';

interface CollapsibleProps {
    title?: string;
    defaultOpen?: boolean;
    children: ReactNode | ReactNode[];
    isOpen?: boolean;
    onOpenChange?: (isOpen: boolean) => void;
    titleSize?: TitleSize;
    render?: ({ isOpen }: { isOpen: boolean }) => ReactNode;
    hideArrow?: boolean;
    hideBorder?: boolean;
}

export function Collapsible({
    title = '',
    children,
    defaultOpen,
    isOpen,
    onOpenChange,
    titleSize = TitleSize.Small,
    render,
    hideArrow,
    hideBorder,
}: CollapsibleProps) {
    const [open, setOpen] = useState(isOpen ?? defaultOpen ?? false);

    function handleOpenChange(isOpen: boolean) {
        setOpen(isOpen);
        onOpenChange?.(isOpen);
    }

    return (
        <Accordion hideBorder={hideBorder}>
            <AccordionHeader
                hideBorder={hideBorder}
                hideArrow={hideArrow}
                isExpanded={isOpen ?? open}
                onToggle={() => handleOpenChange(!open)}
            >
                {render ? render({ isOpen: open }) : <Title size={titleSize} title={title} />}
            </AccordionHeader>
            <AccordionContent isExpanded={isOpen ?? open}>{children}</AccordionContent>
        </Accordion>
    );
}
