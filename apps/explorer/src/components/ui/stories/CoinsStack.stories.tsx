// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClientProvider } from '@iota/dapp-kit';
import { type Meta, type StoryObj } from '@storybook/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { CoinsStack, type CoinsStackProps } from '~/components/ui';

export default {
    component: CoinsStack,
    decorators: [
        (Story) => (
            <QueryClientProvider client={new QueryClient()}>
                <IotaClientProvider>
                    <Story />
                </IotaClientProvider>
            </QueryClientProvider>
        ),
    ],
} as Meta;

export const Default: StoryObj<CoinsStackProps> = {
    args: {
        coinTypes: [
            '0x2::iota::IOTA',
            '0xc0d761079b1e7fa4dbd8a881b7464cf8c400c0de72460fdf8ca44e3f1842715e::iota_inu::IOTA_INU',
            'random',
        ],
    },
};
