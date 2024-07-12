// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { ButtonSegment, SegmentedButton } from '@/components';
import { SegmentedButtonType } from '@/lib/components';
import { ComponentProps, useState } from 'react';

const DotIcon = () => (
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none">
        <path
            d="M7.33301 6C7.33301 6.73638 6.73605 7.33333 5.99967 7.33333C5.26329 7.33333 4.66634 6.73638 4.66634 6C4.66634 5.26362 5.26329 4.66667 5.99967 4.66667C6.73605 4.66667 7.33301 5.26362 7.33301 6Z"
            fill="currentColor"
        />
        <path
            d="M9.99967 7.33333C10.7361 7.33333 11.333 6.73638 11.333 6C11.333 5.26362 10.7361 4.66667 9.99967 4.66667C9.2633 4.66667 8.66634 5.26362 8.66634 6C8.66634 6.73638 9.2633 7.33333 9.99967 7.33333Z"
            fill="currentColor"
        />
        <path
            fill-rule="evenodd"
            clip-rule="evenodd"
            d="M1.33301 5.33333C1.33301 3.1242 3.12387 1.33333 5.33301 1.33333H10.6663C12.8755 1.33333 14.6663 3.1242 14.6663 5.33333V10.6667C14.6663 12.8758 12.8755 14.6667 10.6663 14.6667H5.33301C3.12387 14.6667 1.33301 12.8758 1.33301 10.6667V5.33333ZM2.66634 5.33333C2.66634 3.86058 3.86025 2.66667 5.33301 2.66667H10.6663C12.1391 2.66667 13.333 3.86057 13.333 5.33333V7.914C9.94892 9.47277 6.05043 9.47277 2.66634 7.914V5.33333ZM2.66634 9.36754V10.6667C2.66634 12.1394 3.86025 13.3333 5.33301 13.3333H10.6663C12.1391 13.3333 13.333 12.1394 13.333 10.6667V9.36754C9.9161 10.766 6.08325 10.766 2.66634 9.36754Z"
            fill="currentColor"
        />
    </svg>
);
const meta = {
    component: SegmentedButton,
    tags: ['autodocs'],
    render: (props) => {
        const [elements, setElements] = useState<ComponentProps<typeof ButtonSegment>[]>([
            { label: 'Label 1', selected: true },
            { label: 'Label 2', icon: <DotIcon /> },
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
                            label={element.label}
                            icon={element.icon}
                            selected={element.selected}
                            disabled={element.disabled}
                            onClick={() => handleElementClick(index)}
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
