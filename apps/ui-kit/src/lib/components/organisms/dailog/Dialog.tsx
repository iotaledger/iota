// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as RadixDialog from '@radix-ui/react-dialog';
import cx from 'classnames';
import * as React from 'react';

const Dialog = RadixDialog.Root;
const DialogTrigger = RadixDialog.Trigger;
const DialogClose = RadixDialog.Close;

const DialogOverlay = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Overlay>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Overlay>
>(({ className, ...props }, ref) => (
    <RadixDialog.Overlay
        ref={ref}
        className={cx('fixed inset-0 z-[99998] bg-primary-60 backdrop-blur-lg', className)}
        {...props}
    />
));
DialogOverlay.displayName = RadixDialog.Overlay.displayName;

const DialogContent = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Content>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Content> & {
        containerId?: string;
    }
>(({ className, containerId, ...props }, ref) => {
    return (
        <RadixDialog.Portal
            container={containerId ? document.getElementById(containerId) : undefined}
        >
            <DialogOverlay />
            <RadixDialog.Content
                ref={ref}
                className={cx(
                    'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] absolute left-1/2 top-1/2 z-[99999] flex max-h-[60vh] w-80 max-w-[85vw] -translate-x-1/2 -translate-y-1/2 flex-col justify-center gap-3 overflow-hidden rounded-xl bg-primary-100',
                    className,
                )}
                {...props}
            />
        </RadixDialog.Portal>
    );
});
DialogContent.displayName = RadixDialog.Content.displayName;

const DialogHeader = ({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
    <div className={cx('flex flex-col gap-1.5 text-center', className)} {...props} />
);

const DialogFooter = ({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
    <div className={cx('mt-3', className)} {...props} />
);

const DialogTitle = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Title>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Title>
>(({ className, ...props }, ref) => (
    <RadixDialog.Title
        ref={ref}
        className={cx('', className)}
        // className={cx('text-semibold text-heading6 text-gray-90 m-0', className)}
        {...props}
    />
));
DialogTitle.displayName = RadixDialog.Title.displayName;

const DialogDescription = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Description>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Description>
>(({ className, ...props }, ref) => (
    <RadixDialog.Description ref={ref} className={cx('text-body-sm', className)} {...props} />
));
DialogDescription.displayName = RadixDialog.Description.displayName;

export {
    Dialog,
    DialogClose,
    DialogTrigger,
    DialogContent,
    DialogHeader,
    DialogFooter,
    DialogTitle,
    DialogDescription,
};
