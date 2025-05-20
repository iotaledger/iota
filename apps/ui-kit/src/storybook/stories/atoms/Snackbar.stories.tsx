// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react';
import type { SnackbarProps } from '@/components/atoms';
import { Snackbar, SnackbarType, Button } from '@/components/atoms';

const meta: Meta<SnackbarProps> = {
    component: Snackbar,
    tags: ['autodocs'],
} satisfies Meta<typeof Snackbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        type: SnackbarType.Default,
        text: 'Test message',
        showClose: false,
        duration: 2000,
    },
    argTypes: {},
    render: (props) => {
        const [isOpen, setIsOpen] = useState(false);

        const onClose = () => {
            setIsOpen(false);
        };

        return (
            <>
                <Button
                    onClick={() => setIsOpen(!isOpen)}
                    text={isOpen ? 'Close Snackbar' : 'Open Snackbar'}
                />
                {isOpen && <Snackbar {...props} onClose={onClose} />}
            </>
        );
    },
};
