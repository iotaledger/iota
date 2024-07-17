// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { CardImage, ImageType, ImageVariant } from '@/components/molecules/card';

const meta = {
    component: CardImage,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div>
                <CardImage
                    type={props.type}
                    variant={props.variant}
                    url={props.url}
                    iconName={props.iconName}
                />
            </div>
        );
    },
} satisfies Meta<typeof CardImage>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        type: ImageType.Placeholder,
        variant: ImageVariant.Rounded,
        url: 'https://via.placeholder.com/150.png',
    },
    argTypes: {},
};
