// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ListItem } from '@/components';
import { Seed } from '@iota/apps-ui-icons';
import type { Meta, StoryObj } from '@storybook/react';

const meta = {
    component: ListItem,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex flex-col">
                <ListItem {...props}>
                    <div className="flex flex-row items-center gap-2">
                        <Seed />
                        <div>Item 1</div>
                    </div>
                </ListItem>
            </div>
        );
    },
} satisfies Meta<typeof ListItem>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    argTypes: {
        showRightIcon: {
            control: 'boolean',
        },
        isDisabled: {
            control: 'boolean',
        },
        hideBottomBorder: {
            control: 'boolean',
        },
    },
};
