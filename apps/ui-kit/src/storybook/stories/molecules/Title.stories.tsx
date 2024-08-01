// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import * as ButtonStory from '../atoms/Button.stories';
import { Title, TooltipPosition } from '@/lib/components';

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
        tooltipText: {
            control: 'text',
        },
        button: {
            control: 'object',
        },
        tooltipPosition: {
            control: {
                type: 'select',
                options: Object.values(TooltipPosition),
            },
        },
    },
};
