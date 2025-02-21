// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Select, SelectOption } from '@/components/molecules/select/Select';
import { useState } from 'react';
import { IotaLogoMark, PlaceholderReplace } from '@iota/apps-ui-icons';

const meta: Meta<typeof Select> = {
    component: Select,
    tags: ['autodocs'],
} satisfies Meta<typeof Select>;

export default meta;

type Story = StoryObj<typeof meta>;

const dropdownOptions: SelectOption[] = ['Option 1', 'Option 2', 'Option 3', 'Invalid Option'];

export const Default: Story = {
    args: {
        label: 'Select Input',
        supportingText: 'Info',
        caption: 'Caption',
        placeholder: 'Placeholder',
        options: dropdownOptions,
    },
    argTypes: {
        placeholder: {
            control: {
                type: 'text',
            },
        },
    },
    render: (args) => {
        const [selected, setSelected] = useState('Option 1');
        const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);

        const onChange = (id: string) => {
            if (id === 'Invalid Option') {
                setErrorMessage('Invalid Option Selected');
            } else {
                setSelected(id);
                setErrorMessage(undefined);
            }
        };

        return (
            <div className="h-60">
                <Select
                    {...args}
                    value={selected}
                    onValueChange={onChange}
                    errorMessage={errorMessage}
                />
            </div>
        );
    },
};

export const CustomOptions: Story = {
    args: {
        label: 'Send Coins',
        placeholder: 'Select a coin',
        options: [],
    },
    render: ({ options, ...args }) => {
        const [selected, setSelected] = useState('iota');

        const customOptions: SelectOption[] = [
            {
                id: 'iota',
                renderLabel: () => (
                    <div className="flex items-center gap-2">
                        <IotaLogoMark />
                        IOTA
                    </div>
                ),
            },
            {
                id: 'other coin',
                renderLabel: () => (
                    <div className="flex items-center gap-2">
                        <PlaceholderReplace />
                        Other Coin
                    </div>
                ),
            },
        ];

        return (
            <div className="h-60">
                <Select
                    {...args}
                    value={selected}
                    onValueChange={setSelected}
                    options={customOptions}
                />
            </div>
        );
    },
};
