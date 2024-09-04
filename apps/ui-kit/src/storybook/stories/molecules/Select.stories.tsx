// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Select, SelectOption } from '@/components/molecules/select/Select';
import { useState } from 'react';
import { IotaLogoMark, PlaceholderReplace } from '@iota/ui-icons';

const meta: Meta<typeof Select> = {
    component: Select,
    tags: ['autodocs'],
} satisfies Meta<typeof Select>;

export default meta;

type Story = StoryObj<typeof meta>;

const INVALID_OPTION = {
    id: 'Invalid Option',
    value: 'Invalid Option',
};

const options = new Array(4).fill(0).map((_, index, arr) => {
    const isLastItem = index === arr.length - 1;

    return isLastItem
        ? INVALID_OPTION
        : { id: `Option ${index + 1}`, value: `Option ${index + 1}` };
});

export const Default: Story = {
    args: {
        label: 'Select Input',
        supportingText: 'Info',
        caption: 'Caption',
        value: options[0].value,
        options: new Array(4).fill(0).map((_, index, arr) => {
            const isLastItem = index === arr.length - 1;

            return isLastItem
                ? INVALID_OPTION
                : { id: `Option ${index + 1}`, value: `Option ${index + 1}` };
        }),
    },
    argTypes: {
        value: {
            control: {
                type: 'text',
            },
        },
    },
    render: ({ value, ...args }) => {
        const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);

        function onChange(option: SelectOption['id']) {
            if (option === INVALID_OPTION.id) {
                setErrorMessage('Invalid Option Selected');
            } else {
                setErrorMessage(undefined);
            }
        }

        return (
            <div className="h-60">
                <Select {...args} onValueChange={onChange} errorMessage={errorMessage} />
            </div>
        );
    },
};

export const CustomOptions: Story = {
    args: {
        label: 'Send Coins',
        value: 'Select a coin',
        options: [
            {
                id: 'iota',
                renderValue: (
                    <div className="flex items-center gap-2">
                        <IotaLogoMark />
                        IOTA
                    </div>
                ),
            },
            {
                id: 'smr',
                renderValue: (
                    <div className="flex items-center gap-2">
                        <PlaceholderReplace />
                        SMR
                    </div>
                ),
            },
        ],
    },
    render: (args) => {
        return (
            <div className="h-60">
                <Select {...args} />
            </div>
        );
    },
};
