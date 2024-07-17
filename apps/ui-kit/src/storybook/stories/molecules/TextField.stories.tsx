// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { TextField, TextFieldType } from '@/components/molecules/text-field';
import { PlaceholderReplace } from '@iota/ui-icons';
import { ComponentProps, useEffect, useState } from 'react';

type CustomStoryProps = {
    withLeadingIcon?: boolean;
};

function TextFieldStory({
    withLeadingIcon,
    ...props
}: ComponentProps<typeof TextField> & CustomStoryProps): JSX.Element {
    const [value, setValue] = useState(props.value ?? '');

    useEffect(() => {
        setValue(props.value ?? '');
    }, [props.value]);

    return (
        <TextField
            {...props}
            onChange={(value) => setValue(value)}
            value={value}
            leadingIcon={withLeadingIcon ? <PlaceholderReplace /> : undefined}
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
        type: TextFieldType.Text,
    },
    argTypes: {
        amountCounter: {
            control: {
                type: 'text',
            },
        },
        leadingIcon: {
            control: {
                type: 'none',
            },
        },
    },
    render: (props) => <TextFieldStory {...props} />,
};

export const WithLeadingElement: Story = {
    args: {
        type: TextFieldType.Text,
        placeholder: 'Placeholder',
        amountCounter: '10',
        caption: 'Caption',
    },
    render: (props) => <TextFieldStory {...props} withLeadingIcon />,
};
