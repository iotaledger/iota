// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Accordion, AccordionContent, AccordionHeader, Title, TitleSize } from '@iota/apps-ui-kit';
import { useState, type ReactNode } from 'react';

interface CollapsibleProps {
    title?: string;
    defaultOpen?: boolean;
    children: ReactNode | ReactNode[];
    shade?: 'lighter' | 'darker';
    isOpen?: boolean;
    onOpenChange?: (isOpen: boolean) => void;
    titleSize?: TitleSize;
    renderHeader?: ({ isOpen }: { isOpen: boolean }) => ReactNode;
    hideArrow?: boolean;
    hideBorder?: boolean;
}

export function Collapsible({
    title = '',
    children,
    defaultOpen,
    isOpen,
    onOpenChange,
    shade = 'lighter',
    titleSize = TitleSize.Small,
    renderHeader,
    hideArrow,
    hideBorder,
}: CollapsibleProps) {
    const [open, setOpen] = useState(isOpen ?? defaultOpen ?? false);

    const handleOpenChange = (isOpen: boolean) => {
        setOpen(isOpen);
        onOpenChange?.(isOpen);
    };

    return (
        <Accordion hideBorder={hideBorder}>
            <AccordionHeader
                hideBorder={hideBorder}
                hideArrow={hideArrow}
                isExpanded={isOpen ?? open}
                onToggle={() => handleOpenChange(!open)}
            >
                {renderHeader ? (
                    renderHeader({ isOpen: open })
                ) : (
                    <Title size={titleSize} title={title} />
                )}
            </AccordionHeader>
            <AccordionContent isExpanded={isOpen ?? open}>{children}</AccordionContent>
        </Accordion>
    );
}
