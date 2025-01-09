// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState, type ReactNode } from 'react';
import { Accordion, AccordionContent, AccordionHeader, Title, TitleSize } from '@iota/apps-ui-kit';

interface CollapsibleProps extends React.PropsWithChildren {
    title?: string;
    defaultOpen?: boolean;
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
