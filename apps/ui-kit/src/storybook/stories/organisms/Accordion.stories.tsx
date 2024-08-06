// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import { Accordion, KeyValueInfo } from '@/components';

const meta: Meta<typeof Accordion> = {
    component: Accordion,
    tags: ['autodocs'],
    render: (props) => {
        const [isExpanded, setIsExpanded] = useState(true);

        const onToggle = () => {
            setIsExpanded(!isExpanded);
        };

        return (
            <div>
                <Accordion {...props} isExpanded={isExpanded} onToggle={onToggle}>
                    <div className="flex flex-col gap-2">
                        <KeyValueInfo keyText={'Label'} valueText={'Value'} />
                        <KeyValueInfo keyText={'Label'} valueText={'Value'} />
                        <KeyValueInfo keyText={'Label'} valueText={'Value'} />
                    </div>
                </Accordion>
            </div>
        );
    },
} satisfies Meta<typeof Accordion>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Accordion Title',
        isExpanded: true,
    },
    argTypes: {},
};
