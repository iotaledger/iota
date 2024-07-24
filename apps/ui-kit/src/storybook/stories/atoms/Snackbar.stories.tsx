// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Snackbar, SnackbarProps, SnackbarType } from '@/components/atoms';

const meta: Meta<
    SnackbarProps & {
        actionLabel: string;
    }
> = {
    component: Snackbar,
    tags: ['autodocs'],
} satisfies Meta<typeof Snackbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        type: SnackbarType.Default,
        isOpen: true,
        message: 'Test message',
        actionLabel: 'Action title',
    },
    argTypes: {
        actionLabel: {
            control: 'text',
        },
    },
    render: (props) => {
        return (
            <Snackbar
                {...props}
                action={
                    props.actionLabel
                        ? {
                              label: props.actionLabel,
                              onClick: () => {},
                          }
                        : undefined
                }
            />
        );
    },
};
