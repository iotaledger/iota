// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Address, BadgeType, Panel, PanelSize, PanelTitleSize } from '@/components';

const meta = {
    component: Panel,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <Panel {...props}>
                <div className="flex flex-col items-start gap-2">
                    <Address text="0x0d7...3f34" isCopyable />
                    <Address text="0x0d7...3f35" isCopyable />
                    <Address text="0x0d7...3f36" isCopyable />
                </div>
            </Panel>
        );
    },
} satisfies Meta<typeof Panel>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        title: 'Your Address',
    },
    argTypes: {
        title: {
            control: 'text',
        },
        titleSize: {
            control: {
                type: 'select',
                options: Object.values(PanelTitleSize),
            },
        },
        badgeType: {
            control: 'select',
            options: Object.values(BadgeType),
        },
        badgeText: {
            control: 'text',
        },
        hasBorder: {
            control: 'boolean',
        },
        size: {
            control: {
                type: 'select',
                options: Object.values(PanelSize),
            },
        },
    },
};
