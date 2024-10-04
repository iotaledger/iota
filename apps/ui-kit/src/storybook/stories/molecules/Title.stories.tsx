// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import * as ButtonStory from '../atoms/Button.stories';
import * as BadgeStory from '../atoms/Badge.stories';
import { Badge, BadgeType, Button, Title, TooltipPosition } from '@/lib/components';

const meta: Meta<typeof Title> = {
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
        tooltipPosition: {
            control: {
                type: 'select',
                options: Object.values(TooltipPosition),
            },
        },
    },
};

export const WithTooltip: Story = {
    args: {
        title: 'Title',
        subtitle: 'Subtitle',
        tooltipText: 'Tooltip',
        tooltipPosition: TooltipPosition.Top,
    },
};

export const WithButton: Story = {
    args: {
        title: 'Title',
        subtitle: 'Subtitle',
    },
    render: (props) => {
        return <Title {...props} trailingElement={<Button {...ButtonStory.Default.args} />} />;
    },
};

export const WithSupportingElement: Story = {
    args: {
        title: 'Title',
    },
    render: (props) => {
        return (
            <Title
                {...props}
                supportingElement={<Badge {...BadgeStory.Default.args} type={BadgeType.Neutral} />}
            />
        );
    },
};
