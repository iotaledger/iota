// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Card } from '@/components/molecules/card/Card';
// import * as ButtonStory from '../atoms/Button.stories';

const meta = {
    component: Card,
    tags: ['autodocs'],
    render: (props) => {
        return <Card {...props} />;
    },
} satisfies Meta<typeof Card>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Title',
        subtitle: 'Subtitle',
    },
    argTypes: {
        title: {
            control: 'text',
        },
        subtitle: {
            control: 'text',
        },
    },
};
