// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { RadioButton } from '@/components/atoms';
import { useEffect, useState } from 'react';

const meta: Meta<typeof RadioButton> = {
    component: RadioButton,
    tags: ['autodocs'],
    render: (props) => {
        const { isChecked } = props;
        const [checked, setIsChecked] = useState<boolean>(isChecked ?? false);

        useEffect(() => {
            setIsChecked(isChecked ?? false);
        }, [isChecked]);

        return (
            <RadioButton
                {...props}
                isChecked={checked}
                onChange={(e) => setIsChecked(e.target.checked)}
            />
        );
    },
} satisfies Meta<typeof RadioButton>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Radio Button',
        isChecked: false,
        isDisabled: false,
    },
    argTypes: {
        label: {
            control: 'text',
        },
        isChecked: {
            control: 'boolean',
        },
        isDisabled: {
            control: 'boolean',
        },
    },
};
