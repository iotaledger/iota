// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { SelectorField, SelectorOption } from '@/components/molecules/selector-field/SelectorField';
import { useState } from 'react';
import { IotaLogoMark, PlaceholderReplace } from '@iota/ui-icons';

const meta = {
    component: SelectorField,
    tags: ['autodocs'],
} satisfies Meta<typeof SelectorField>;

export default meta;

type Story = StoryObj<typeof meta>;

const dropdownOptions: SelectorOption[] = ['Option 1', 'Option 2', 'Option 3', 'Invalid Option'];

export const Default: Story = {
    args: {
        label: 'Selector Field',
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
        const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);

        const onChange = (id: string) => {
            if (id === 'Invalid Option') {
                setErrorMessage('Invalid Option Selected');
            } else {
                setErrorMessage(undefined);
            }
        };

        return (
            <div className="h-60">
                <SelectorField {...args} onValueChange={onChange} errorMessage={errorMessage} />
            </div>
        );
    },
};

export const CustomOptions: Story = {
    args: {
        label: 'Selector Field',
        supportingText: 'Info',
        placeholder: 'Select a coin',
        options: [],
    },
    render: ({ options, ...args }) => {
        const customOptions: SelectorOption[] = [
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
                id: 'smr',
                renderLabel: () => (
                    <div className="flex items-center gap-2">
                        <PlaceholderReplace />
                        SMR
                    </div>
                ),
            },
        ];

        return (
            <div className="h-60">
                <SelectorField {...args} options={customOptions} />
            </div>
        );
    },
};
