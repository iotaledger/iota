// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@/components';
import { Info } from '@iota/apps-ui-icons';

const meta: Meta<typeof InfoBox> = {
    component: InfoBox,
    tags: ['autodocs'],
    render: (props) => {
        return <InfoBox {...props} />;
    },
} satisfies Meta<typeof InfoBox>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        type: InfoBoxType.Default,
        title: 'Title',
        supportingText: 'This is an info box',
        style: InfoBoxStyle.Default,
        icon: <Info />,
    },
    argTypes: {
        type: {
            control: {
                type: 'select',
                options: Object.values(InfoBoxType),
            },
        },
        title: {
            control: {
                type: 'text',
            },
        },
        supportingText: {
            control: {
                type: 'text',
            },
        },
        icon: {
            control: 'none',
        },
        style: {
            control: {
                type: 'select',
                options: Object.values(InfoBoxStyle),
            },
        },
    },
};

export const WithoutIcon: Story = {
    args: {
        type: InfoBoxType.Default,
        title: 'Title',
        supportingText: 'This is an info box without an icon',
        style: InfoBoxStyle.Default,
    },
    render: (props) => {
        return <InfoBox {...props} />;
    },
};
