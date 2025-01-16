// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import {
    ButtonSegment,
    ButtonSegmentType,
    SegmentedButton,
    SegmentedButtonType,
} from '@/components';
import { ComponentProps, useState } from 'react';
import { PlaceholderReplace } from '@iota/apps-ui-icons';

const meta: Meta<typeof SegmentedButton> = {
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
                <SegmentedButton type={props.type} shape={props.shape}>
                    {elements.map((element, index) => (
                        <ButtonSegment
                            key={element.label}
                            type={props.shape}
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
        shape: {
            control: {
                type: 'select',
                options: Object.values(ButtonSegmentType),
            },
        },
    },
};

export const Underlined: Story = {
    args: {
        type: SegmentedButtonType.Transparent,
        shape: ButtonSegmentType.Underlined,
    },
    render: (props) => {
        const [elements, setElements] = useState<ComponentProps<typeof ButtonSegment>[]>([
            { label: 'Label 1', selected: true, type: ButtonSegmentType.Underlined },
            { label: 'Label 2', icon: <PlaceholderReplace />, type: ButtonSegmentType.Underlined },
            { label: 'Label 3', type: ButtonSegmentType.Underlined },
            { label: 'Label 4', disabled: true, type: ButtonSegmentType.Underlined },
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
                <SegmentedButton type={props.type} shape={props.shape}>
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
};
