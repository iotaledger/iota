// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Accordion, AccordionContent, AccordionHeader, Title, TitleSize } from '@iota/apps-ui-kit';
import { type ReactNode, useState } from 'react';

interface CollapsibleSectionProps {
    children: ReactNode;
    defaultOpen?: boolean;
    title?: string;
    open?: boolean;
    onOpenChange?: (open: boolean) => void;
    render?: ({ isOpen }: { isOpen: boolean }) => ReactNode;
    titleSize?: TitleSize;
    hideArrow?: boolean;
    hideBorder?: boolean;
}

export function CollapsibleSection({
    title,
    defaultOpen = true,
    children,
    open,
    onOpenChange,
    render,
    titleSize = TitleSize.Small,
    hideArrow,
    hideBorder,
}: CollapsibleSectionProps): JSX.Element {
    const [isOpen, setIsOpen] = useState(defaultOpen);
    const isOpenState = typeof open === 'undefined' ? isOpen : open;
    const setOpenState = typeof onOpenChange === 'undefined' ? setIsOpen : onOpenChange;
    return (
        <div className="px-md--rs pb-lg pt-xs">
            <Accordion hideBorder={hideBorder}>
                {title && (
                    <AccordionHeader
                        hideBorder={hideBorder}
                        hideArrow={hideArrow}
                        isExpanded={isOpen}
                        onToggle={() => setOpenState(!isOpenState)}
                    >
                        {render ? (
                            render({ isOpen: isOpenState })
                        ) : (
                            <Title size={titleSize} title={title ?? ''} />
                        )}
                    </AccordionHeader>
                )}

                <AccordionContent isExpanded={isOpenState}>{children}</AccordionContent>
            </Accordion>
        </div>
    );
}
