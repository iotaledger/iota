// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { ButtonSegment, SegmentedButton } from '@/components';
import { SegmentedButtonType } from '@/lib/components';
import { ComponentProps, useState } from 'react';

const meta = {
    component: SegmentedButton,
    tags: ['autodocs'],
    render: (props) => {
        const [elements, setElements] = useState<ComponentProps<typeof ButtonSegment>[]>(
            props.elements || [],
        );

        const handleElementClick = (clickedElementLabel: string) => {
            const updatedElements = elements.map((element) => ({
                ...element,
                selected: element.label === clickedElementLabel,
            }));
            setElements(updatedElements);
        };

        return (
            <div className="flex flex-col items-start">
                <SegmentedButton
                    type={props.type}
                    elements={elements}
                    onSelected={(selectedElement) => handleElementClick(selectedElement.label)}
                />
            </div>
        );
    },
} satisfies Meta<typeof SegmentedButton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        elements: [
            { label: 'Label 1', selected: true },
            { label: 'Label 2' },
            { label: 'Label 3', disabled: true },
            { label: 'Label 4' },
        ],
    },
    argTypes: {
        elements: {
            control: 'object',
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(SegmentedButtonType),
            },
        },
    },
};
