// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Button } from '@/components/atoms/button/Button';
import { ButtonSize, ButtonType } from '@/components/atoms/button';
import { Close } from '@iota/ui-icons';

const meta: Meta<typeof Button> = {
    component: Button,
    tags: ['autodocs'],
    render: (props) => {
        return <Button {...props} />;
    },
} satisfies Meta<typeof Button>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        text: 'Button',
    },
    argTypes: {
        text: {
            control: 'text',
        },
        size: {
            control: {
                type: 'select',
                options: Object.values(ButtonSize),
            },
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(ButtonType),
            },
        },
        disabled: {
            control: 'boolean',
        },
    },
};

export const IconOnlyButton: Story = {
    args: {
        type: ButtonType.Icon,
        icon: <Close className="text-neutral-12 dark:text-neutral-90" />,
        onClick: () => {
            alert('Button clicked');
        },
    },
    render: (props) => {
        return <Button {...props} />;
    },
};
