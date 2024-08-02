// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import { Snackbar, SnackbarProps, SnackbarType } from '@/components/atoms';

const meta: Meta<SnackbarProps> = {
    component: Snackbar,
    tags: ['autodocs'],
} satisfies Meta<typeof Snackbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        type: SnackbarType.Default,
        isOpen: true,
        text: 'Test message',
    },
    argTypes: {},
    render: (props) => {
        const [isOpen, setIsOpen] = useState(false);

        const onClose = () => {
            setIsOpen(false);
        };

        return (
            <>
                <button onClick={() => setIsOpen(true)}>Open Snackbar</button>
                <Snackbar {...props} isOpen={isOpen} onClose={onClose} />
            </>
        );
    },
};
