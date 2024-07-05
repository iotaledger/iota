// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Icon } from '@/components/icon/Icon';
import { IconEnum } from '@/lib';

const meta = {
    component: Icon,
    tags: ['autodocs'],
    render: (props, context) => {
        const { darkmode } = context;
        return (
            <div className={darkmode ? 'text-neutral-90' : 'text-neutral-20'}>
                <Icon {...props} />
            </div>
        );
    },
} satisfies Meta<typeof Icon>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        icon: IconEnum.Assets,
    },
    argTypes: {
        width: {
            control: {
                type: 'range',
                min: 1,
                max: 100,
                step: 1,
            },
        },
        height: {
            control: {
                type: 'range',
                min: 1,
                max: 100,
                step: 1,
            },
        },
    },
};
