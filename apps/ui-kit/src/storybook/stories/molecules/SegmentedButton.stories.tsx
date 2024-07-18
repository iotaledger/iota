// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { ButtonSegment, SegmentedButton } from '@/components';
import { SegmentedButtonType } from '@/lib/components';
import { ComponentProps, useState } from 'react';
import { PlaceholderReplace } from '@iota/ui-icons';

const meta = {
    component: SegmentedButton,
    tags: ['autodocs'],
    render: (props) => {
        const [elements, setElements] = useState<ComponentProps<typeof ButtonSegment>[]>([
            { label: 'Label 1', selected: true },
            { label: 'Label 2', icon: <PlaceholderReplace /> },
            { label: 'Label 3', disabled: true },
            { label: 'Label 4' },
        ]);

        const handleElementClick = (clickedIndex: number) => {
            const updatedElements = elements.map((element, index) => ({
                ...element,
                selected: index === clickedIndex,
            }));
            setElements(updatedElements);
        };

        return (
            <div className="flex flex-col items-start">
                <SegmentedButton type={props.type}>
                    {elements.map((element, index) => (
                        <ButtonSegment
                            key={element.label}
                            onClick={() => handleElementClick(index)}
                            {...element}
                        />
                    ))}
                </SegmentedButton>
            </div>
        );
    },
} satisfies Meta<typeof SegmentedButton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    argTypes: {
        type: {
            control: {
                type: 'select',
                options: Object.values(SegmentedButtonType),
            },
        },
    },
};
