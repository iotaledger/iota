// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { Address, Panel, Title } from '@/components';

const meta = {
    component: Panel,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <Panel {...props}>
                <div className="flex flex-col items-start gap-2 px-md--rs py-sm--rs">
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
    argTypes: {
        title: {
            control: 'text',
        },
        hasBorder: {
            control: 'boolean',
        },
    },
};

export const WithTitle: Story = {
    args: {
        title: <Title title="Title" subtitle="subtitle" />,
    },
    argTypes: {
        title: {
            control: 'text',
        },
        hasBorder: {
            control: 'boolean',
        },
    },
};
