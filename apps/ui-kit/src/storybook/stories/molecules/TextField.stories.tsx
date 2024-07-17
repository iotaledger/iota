// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { TextField } from '@/components/molecules/textfield/TextField';
import { PlaceholderReplace } from '@iota/ui-icons';
import { ComponentProps, useState } from 'react';

type CustomStoryProps = {
    withLeadingElement?: boolean;
    withPlaceholder?: boolean;
};

function TextFieldStory({
    placeholder,
    withLeadingElement,
    withPlaceholder,
    ...props
}: ComponentProps<typeof TextField> & CustomStoryProps): JSX.Element {
    const [value, setValue] = useState('');
    return (
        <TextField
            {...props}
            onChange={(value) => setValue(value)}
            value={value}
            onResetClick={() => setValue('')}
            leadingElement={withLeadingElement ? <PlaceholderReplace /> : undefined}
            placeholder={placeholder ?? (withPlaceholder ? 'Placeholder' : undefined)}
        />
    );
}

const meta = {
    component: TextField,
    tags: ['autodocs'],
} satisfies Meta<typeof TextField>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        caption: 'Caption',
    },
    argTypes: {
        amountCounter: {
            control: {
                type: 'text',
            },
        },
        leadingElement: {
            control: {
                type: 'none',
            },
        },
    },
    render: (props) => <TextFieldStory {...props} />,
};

export const WithLeadingElement: Story = {
    render: (props) => <TextFieldStory {...props} withLeadingElement withPlaceholder />,
};
