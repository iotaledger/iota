// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { PropsWithChildren, useState } from 'react';
import {
    AccordionHeaderProps,
    Accordion,
    AccordionHeader,
    AccordionContent,
    TitleSize,
    Title,
} from '@iota/apps-ui-kit';

type CollapsibleProps = {
    title: string;
    hideBorder?: boolean;
    defaultExpanded?: boolean;
    headerProps?: AccordionHeaderProps;
    titleSize?: TitleSize;
};

export function Collapsible({
    title,
    children,
    defaultExpanded = false,
    hideBorder,
    titleSize = TitleSize.Small,
}: PropsWithChildren<CollapsibleProps>) {
    const [isExpanded, setIsExpanded] = useState(defaultExpanded);
    return (
        <Accordion hideBorder={hideBorder}>
            <AccordionHeader
                hideBorder={hideBorder}
                isExpanded={isExpanded}
                onToggle={() => setIsExpanded(!isExpanded)}
            >
                <Title size={titleSize} title={title} />
            </AccordionHeader>
            <AccordionContent isExpanded={isExpanded}>{children}</AccordionContent>
        </Accordion>
    );
}
