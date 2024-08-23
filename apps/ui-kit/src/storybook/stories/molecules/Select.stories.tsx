// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Select, SelectOption } from '@/components/molecules/select/Select';
import { ReactNode, useState } from 'react';
import { IotaLogoMark, PlaceholderReplace } from '@iota/ui-icons';

const meta: Meta<typeof Select> = {
    component: Select,
    tags: ['autodocs'],
} satisfies Meta<typeof Select>;

export default meta;

type Story = StoryObj<typeof meta>;

const INVALID_OPTION = {
    id: 'Invalid Option',
    displayElement: 'Invalid Option',
};

const options = new Array(4).fill(0).map((_, index, arr) => {
    const isLastItem = index === arr.length - 1;

    return isLastItem
        ? INVALID_OPTION
        : { id: `Option ${index + 1}`, displayElement: `Option ${index + 1}` };
});

export const Default: Story = {
    args: {
        label: 'Select Input',
        supportingText: 'Info',
        caption: 'Caption',
        placeholder: options[0].displayElement,
        options: new Array(4).fill(0).map((_, index, arr) => {
            const isLastItem = index === arr.length - 1;

            return isLastItem
                ? INVALID_OPTION
                : { id: `Option ${index + 1}`, displayElement: `Option ${index + 1}` };
        }),
    },
    argTypes: {
        placeholder: {
            control: {
                type: 'text',
            },
        },
    },
    render: ({ placeholder, ...args }) => {
        const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);
        const [selectorPlaceholder, setSelectorPlaceholder] = useState<ReactNode>(placeholder);

        function onChange(option: SelectOption) {
            if (option.id === INVALID_OPTION.id) {
                setErrorMessage('Invalid Option Selected');
            } else {
                setSelectorPlaceholder(option.displayElement);
                setErrorMessage(undefined);
            }
        }

        return (
            <div className="h-60">
                <Select
                    {...args}
                    placeholder={selectorPlaceholder}
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
        options: [
            {
                id: 'iota',
                displayElement: (
                    <div className="flex items-center gap-2">
                        <IotaLogoMark />
                        IOTA
                    </div>
                ),
            },
            {
                id: 'smr',
                displayElement: (
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
