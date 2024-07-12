// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ButtonSize } from '@/lib/components';

import type { Meta, StoryObj } from '@storybook/react';

import {
    Button,
    ButtonType,
    Header,
    Dialog,
    DialogContent,
    DialogFooter,
    DialogHeader,
    // DialogTitle,
    DialogDescription,
} from '@/components';

const meta = {
    component: Dialog,
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className="flex h-96">
                <Dialog defaultOpen>
                    <DialogContent>
                        <DialogHeader>
                            <Header
                                title="Connect Ledger Wallet"
                                hasLeftIcon
                                hasRightIcon
                                titleCentered
                            />
                            {/* <DialogTitle>Connect Ledger Wallet</DialogTitle> */}
                        </DialogHeader>
                        <DialogDescription>
                            <div className="flex flex-col items-center">
                                <div className="mt-4.5">Logo</div>
                                <div className="mt-4.5 break-words text-center">
                                    Connect your Ledger device to continue.
                                </div>
                            </div>
                        </DialogDescription>
                        <DialogFooter>
                            <div className="flex w-full flex-row justify-center gap-2 p-md">
                                <Button
                                    size={ButtonSize.Small}
                                    type={ButtonType.Outlined}
                                    text="Cancel"
                                />
                                <Button size={ButtonSize.Small} text="Connect" />
                            </div>
                        </DialogFooter>
                    </DialogContent>
                </Dialog>
            </div>
        );
    },
} satisfies Meta<typeof Dialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    // args: {
    //     title: 'Title',
    //     subtitle: 'Subtitle',
    //     button: {
    //         ...ButtonStory.Default.args,
    //     },
    // },
    // argTypes: {
    //     title: {
    //         control: 'text',
    //     },
    //     subtitle: {
    //         control: 'text',
    //     },
    //     info: {
    //         control: 'text',
    //     },
    //     button: {
    //         control: 'object',
    //     },
    // },
};
