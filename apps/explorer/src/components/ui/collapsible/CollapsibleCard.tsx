// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Accordion,
    AccordionContent,
    AccordionHeader,
    Divider,
    Panel,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import clsx from 'clsx';
import { type ReactNode, useState } from 'react';

export interface CollapsibleCardProps {
    children: ReactNode;
    title?: string;
    footer?: ReactNode;
    collapsible?: boolean;
    initialClose?: boolean;
    titleSize?: TitleSize;
    hideArrow?: boolean;
    hideBorder?: boolean;
    render?: ({ isOpen }: { isOpen: boolean }) => ReactNode;
    supportingTitleElement?: ReactNode;
    isTransparentPanel?: boolean;
}

export function CollapsibleCard({
    title,
    footer,
    collapsible,
    children,
    initialClose,
    titleSize = TitleSize.Medium,
    hideArrow,
    hideBorder,
    render,
    supportingTitleElement,
    isTransparentPanel,
}: CollapsibleCardProps) {
    const [open, setOpen] = useState(!initialClose);
    return collapsible ? (
        <div className="relative w-full">
            <Accordion hideBorder={hideBorder}>
                <AccordionHeader
                    hideArrow={hideArrow}
                    isExpanded={open}
                    onToggle={() => setOpen(!open)}
                >
                    {render ? (
                        render({ isOpen: open })
                    ) : (
                        <Title
                            size={titleSize}
                            title={title ?? ''}
                            supportingElement={supportingTitleElement}
                        />
                    )}
                </AccordionHeader>
                <AccordionContent isExpanded={open}>{children}</AccordionContent>
                {footer && (
                    <>
                        <Divider />
                        <div className={clsx('rounded-b-2xl')}>{footer}</div>
                    </>
                )}
            </Accordion>
        </div>
    ) : (
        <Panel hasBorder={!hideBorder} bgColor={isTransparentPanel ? 'bg-transparent' : undefined}>
            <Title size={titleSize} title={title ?? ''} />
            <div>{children}</div>
            {footer && (
                <>
                    <Divider />
                    {footer}
                </>
            )}
        </Panel>
    );
}
