// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Meta, type StoryObj } from '@storybook/react';

import { DateCard, type DateCardProps } from '../DateCard';

export default {
    component: DateCard,
} as Meta;

export const defaultAmount: StoryObj<DateCardProps> = {
    args: {
        date: 1667942429177,
    },
};
