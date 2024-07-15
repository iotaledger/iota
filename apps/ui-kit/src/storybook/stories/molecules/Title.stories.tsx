// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Title } from '@/components/molecules/title/Title';
import * as ButtonStory from '../atoms/Button.stories';

const meta = {
    component: Title,
    tags: ['autodocs'],
    render: (props) => {
        return <Title {...props} />;
    },
} satisfies Meta<typeof Title>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Title',
        subtitle: 'Subtitle',
        button: {
            ...ButtonStory.Default.args,
        },
    },
    argTypes: {
        title: {
            control: 'text',
        },
        subtitle: {
            control: 'text',
        },
        info: {
            control: 'text',
        },
        button: {
            control: 'object',
        },
    },
};
