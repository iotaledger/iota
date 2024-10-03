// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState, type ReactNode } from 'react';
import { type Meta, type StoryObj } from '@storybook/react';
import { Accordion, AccordionContent, AccordionHeader, Title, TitleSize } from '@iota/apps-ui-kit';

import { CollapsibleCard, type CollapsibleCardProps } from '~/components/ui';

export default {
    component: CollapsibleCard,
} as Meta;

export const Default: StoryObj<CollapsibleCardProps> = {
    render: (props) => {
        const sections = Array(5)
            .fill(true)
            .map((_, index) => <div key={index}>Section Item {index}</div>);

        return (
            <div className="h-[1000px]">
                <CollapsibleCard collapsible title="Card Title" {...props}>
                    {sections.map((section, index) => (
                        <Section key={index} title={`Section Title ${index}`}>
                            {section}
                        </Section>
                    ))}
                </CollapsibleCard>
            </div>
        );
    },
};

function Section({ children, title }: { children: ReactNode; title: string }) {
    const [isExpanded, setIsExpanded] = useState(true);
    return (
        <div className="px-md--rs pb-lg pt-xs">
            <Accordion>
                <AccordionHeader
                    isExpanded={isExpanded}
                    onToggle={() => setIsExpanded(!isExpanded)}
                >
                    <Title size={TitleSize.Small} title={title} />
                </AccordionHeader>
                <AccordionContent isExpanded>{children}</AccordionContent>
            </Accordion>
        </div>
    );
}
