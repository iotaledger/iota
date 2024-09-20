// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import type { Meta, StoryFn, StoryObj } from '@storybook/react';
import {
    Accordion,
    AccordionContent,
    AccordionHeader,
    Badge,
    BadgeType,
    KeyValueInfo,
    Title,
} from '@/components';

interface CustomStoryProps {
    title: string;
    badgeType: BadgeType;
    badgeLabel: string;
    isExpanded: boolean;
}

const Template: StoryFn<CustomStoryProps> = (args) => {
    const [isExpanded, setIsExpanded] = useState(args.isExpanded);

    const onToggle = () => {
        setIsExpanded(!isExpanded);
    };

    return (
        <Accordion>
            <AccordionHeader isExpanded={isExpanded} onToggle={onToggle}>
                <Title
                    title={args.title}
                    supportingElement={<Badge type={args.badgeType} label={args.badgeLabel} />}
                />
            </AccordionHeader>
            <AccordionContent isExpanded={isExpanded}>
                <div className="flex flex-col gap-2">
                    <KeyValueInfo keyText={'Label'} value="Value" />
                    <KeyValueInfo keyText={'Label'} value="Value" />
                    <KeyValueInfo keyText={'Label'} value="Value" />
                </div>
            </AccordionContent>
        </Accordion>
    );
};

const meta: Meta<CustomStoryProps> = {
    title: 'Organisms/Accordion',
    tags: ['autodocs'],
    argTypes: {
        title: { control: 'text' },
        badgeType: { control: 'select', options: Object.values(BadgeType) },
        badgeLabel: { control: 'text' },
        isExpanded: { control: 'boolean' },
    },
} satisfies Meta<CustomStoryProps>;

export default meta;

export const Default: StoryObj<CustomStoryProps> = {
    render: Template,
    args: {
        title: 'Default Title',
        badgeType: BadgeType.Neutral,
        badgeLabel: 'New',
        isExpanded: true,
    },
};
