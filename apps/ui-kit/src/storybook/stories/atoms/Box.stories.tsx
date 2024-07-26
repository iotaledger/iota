// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import cx from 'classnames';
import { Address, Box, TitleSize } from '@/components';

const meta = {
    component: Box,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <Box {...props}>
                <div className={cx('flex flex-col items-start gap-2', { 'mt-4': props.title })}>
                    <Address text="0x0d7...3f34" isCopyable />
                    <Address text="0x0d7...3f35" isCopyable />
                    <Address text="0x0d7...3f36" isCopyable />
                </div>
            </Box>
        );
    },
} satisfies Meta<typeof Box>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Your Address',
    },
    argTypes: {
        title: {
            control: 'text',
        },
        size: {
            control: {
                type: 'select',
                options: Object.values(TitleSize),
            },
        },
    },
};
