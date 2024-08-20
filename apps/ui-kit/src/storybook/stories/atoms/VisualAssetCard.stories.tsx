// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { VisualAssetCard } from '@/components';
import { MoreHoriz } from '@iota/ui-icons';

const meta: Meta<typeof VisualAssetCard> = {
    component: VisualAssetCard,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="h-64 w-64">
                <VisualAssetCard {...props} />
            </div>
        );
    },
} satisfies Meta<typeof VisualAssetCard>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        assetSrc: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
        altText: 'IOTA Logo',
        icon: <MoreHoriz />,
        onIconClick: () => {
            console.log('Icon clicked');
        },
        onClick: () => {
            console.log('Card clicked');
        },
        assetTitle: 'IOTA Logo',
    },
    argTypes: {
        assetSrc: {
            control: 'text',
        },
        altText: {
            control: 'text',
        },
        icon: {
            control: 'none',
        },
        onIconClick: {
            control: 'none',
        },
        assetType: {
            control: 'none',
        },
        onClick: {
            control: 'none',
        },
        assetTitle: {
            control: 'text',
        },
    },
};
