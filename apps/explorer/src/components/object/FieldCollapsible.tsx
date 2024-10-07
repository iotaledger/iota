// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Accordion,
    AccordionHeader,
    Title,
    AccordionContent,
    type TitleSize,
} from '@iota/apps-ui-kit';
import { type ReactNode } from 'react';

interface FieldCollapsibleProps {
    name?: string;
    open: boolean;
    onOpenChange: (open: boolean) => void;
    children: ReactNode;
    titleSize?: TitleSize;
    hideArrow?: boolean;
    hideBorder?: boolean;
    render?: ({ isOpen }: { isOpen: boolean }) => ReactNode;
}

export function FieldCollapsible({
    name,
    children,
    open,
    titleSize,
    hideArrow,
    hideBorder,
    onOpenChange,
    render,
}: FieldCollapsibleProps): JSX.Element {
    return (
        <Accordion hideBorder={hideBorder}>
            <AccordionHeader
                hideBorder={hideBorder}
                hideArrow={hideArrow}
                isExpanded={open}
                onToggle={() => onOpenChange(!open)}
            >
                {render ? render({ isOpen: open }) : <Title size={titleSize} title={name ?? ''} />}
            </AccordionHeader>
            <AccordionContent isExpanded={open}>{children}</AccordionContent>
        </Accordion>
    );
}
