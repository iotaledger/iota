// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ButtonSize, DialogPosition } from '@/lib/components';

import type { Meta, StoryObj } from '@storybook/react';

import { Button, ButtonType, Header, Dialog, DialogContent, DialogBody } from '@/components';
import { useState } from 'react';

const meta = {
    component: Dialog,
    tags: ['autodocs'],
    render: () => {
        const [open, setOpen] = useState(false);
        return (
            <div className="flex">
                <Button size={ButtonSize.Small} text="Open Dialog" onClick={() => setOpen(true)} />
                <Dialog open={open} onOpenChange={setOpen}>
                    <DialogContent showCloseOnOverlay>
                        <Header
                            title="Connect Ledger Wallet"
                            titleCentered
                            onClose={() => setOpen(false)}
                            onBack={() => setOpen(false)}
                        />
                        <DialogBody>
                            <div className="flex flex-col items-center gap-2">
                                <div className="mt-4.5">Logo</div>
                                <div className="mt-4.5 break-words text-center">
                                    Connect your ledger to your computer, unlock it, and launch the
                                    IOTA app. Click Continue when done.
                                </div>
                                <div className="mt-4.5"> Need more help? View tutorial.</div>
                            </div>
                        </DialogBody>
                        <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                            <Button
                                size={ButtonSize.Small}
                                type={ButtonType.Outlined}
                                text="Cancel"
                                onClick={() => setOpen(false)}
                            />
                            <Button size={ButtonSize.Small} text="Connect" />
                        </div>
                    </DialogContent>
                </Dialog>
            </div>
        );
    },
} satisfies Meta<typeof Dialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const RightDialog: Story = {
    render: () => {
        const [open, setOpen] = useState(false);
        return (
            <div className="flex">
                <Button size={ButtonSize.Small} text="Open Dialog" onClick={() => setOpen(true)} />
                <Dialog open={open} onOpenChange={setOpen}>
                    <DialogContent showCloseOnOverlay position={DialogPosition.Right}>
                        <div className="h-full w-full">
                            <Header
                                title="Connect Ledger Wallet"
                                titleCentered
                                onClose={() => setOpen(false)}
                                onBack={() => setOpen(false)}
                            />
                            <div className="flex h-full w-full flex-col items-center justify-center">
                                <DialogBody>
                                    <div className="flex flex-col items-center gap-2">
                                        <div className="mt-4.5">Logo</div>
                                        <div className="mt-4.5 break-words text-center">
                                            Connect your ledger to your computer, unlock it, and
                                            launch the IOTA app. Click Continue when done.
                                        </div>
                                        <div className="mt-4.5">
                                            {' '}
                                            Need more help? View tutorial.
                                        </div>
                                    </div>
                                </DialogBody>
                                <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                                    <Button
                                        size={ButtonSize.Small}
                                        type={ButtonType.Outlined}
                                        text="Cancel"
                                        onClick={() => setOpen(false)}
                                    />
                                    <Button size={ButtonSize.Small} text="Connect" />
                                </div>
                            </div>
                        </div>
                    </DialogContent>
                </Dialog>
            </div>
        );
    },
};
