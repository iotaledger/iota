// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Dropdown, ListItem } from '@/components';
import type { ComponentProps } from 'react';

const meta = {
    component: Dropdown,
    tags: ['autodocs'],
    render: (props) => {
        return <Dropdown {...props} />;
    },
} satisfies Meta<typeof Dropdown>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {},
    render: (args) => {
        const options: ComponentProps<typeof ListItem>[] = new Array(5).fill(0).map((_, index) => ({
            children: `Option ${index + 1}`,
            onClick: () => {
                alert(`Option ${index + 1} clicked`);
            },
        }));

        return (
            <Dropdown {...args}>
                {options.map((option, index) => (
                    <ListItem key={index} {...option} hideBottomBorder />
                ))}
            </Dropdown>
        );
    },
};
