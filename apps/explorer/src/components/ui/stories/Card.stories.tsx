// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Meta, type StoryObj } from '@storybook/react';

import { Card, type CardProps } from '../Card';

export default {
    component: Card,
} as Meta;

export const Default: StoryObj<CardProps> = {
    render: (props) => <Card {...props}>This is card content.</Card>,
};

export const Small: StoryObj<CardProps> = {
    ...Default,
    args: { spacing: 'sm' },
};

export const Large: StoryObj<CardProps> = {
    ...Default,
    args: { spacing: 'lg' },
};
