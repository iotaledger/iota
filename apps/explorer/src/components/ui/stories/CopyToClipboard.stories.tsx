// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Toaster } from 'react-hot-toast';

import { CopyToClipboard, type CopyToClipboardProps } from '~/components/ui';

import type { Meta, StoryObj } from '@storybook/react';

export default {
    component: CopyToClipboard,
} as Meta;

export const Default: StoryObj<CopyToClipboardProps> = {
    render: () => (
        <div className="flex gap-2">
            <Toaster />
            <CopyToClipboard size="sm" copyText="Copy me!" />
            <CopyToClipboard size="md" copyText="Copy me!" />
            <CopyToClipboard size="lg" copyText="Copy me!" />
        </div>
    ),
};
