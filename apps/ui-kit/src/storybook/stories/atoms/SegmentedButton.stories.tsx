// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { SegmentedButton } from '@/components/atoms/segmented-button/SegmentedButton';
import { SegmentedButtonType } from '@/lib/components/atoms/segmented-button';
import { useState } from 'react';
import { ButtonSegmentProps } from '@/lib/components/atoms/button-segment';

const meta = {
    component: SegmentedButton,
    tags: ['autodocs'],
    render: (props) => {
        console.log('props', props);
        const [elements, setElements] = useState<ButtonSegmentProps[]>(props.elements || []);

        const handleElementClick = (clickedElementLabel: string) => {
            console.log('clickedElementLabel', clickedElementLabel);
            const updatedElements = elements.map((element: ButtonSegmentProps) => ({
                ...element,
                selected: element.label === clickedElementLabel,
            }));
            console.log('updatedElements', updatedElements);
            setElements(updatedElements);
        };

        return (
            <div className="flex flex-col items-start gap-2">
                <SegmentedButton
                    elements={elements}
                    onSelected={(selectedElement: ButtonSegmentProps) =>
                        handleElementClick(selectedElement.label)
                    }
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
