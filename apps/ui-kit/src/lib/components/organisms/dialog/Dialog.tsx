// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as RadixDialog from '@radix-ui/react-dialog';
import * as VisuallyHidden from '@radix-ui/react-visually-hidden';
import cx from 'classnames';
import * as React from 'react';
import { Close } from '@iota/ui-icons';
import { useEffect, useState } from 'react';
import { DialogPosition } from './dialog.enums';

const Dialog = RadixDialog.Root;
const DialogTrigger = RadixDialog.Trigger;
const DialogClose = RadixDialog.Close;

const DialogOverlay = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Overlay>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Overlay> & {
        showCloseIcon?: boolean;
    }
>(({ showCloseIcon, ...props }, ref) => (
    <RadixDialog.Overlay
        ref={ref}
        className="fixed inset-0 z-[99998] bg-shader-neutral-light-48 backdrop-blur-md"
        {...props}
    >
        <DialogClose className={cx('fixed right-3 top-3', { hidden: !showCloseIcon })}>
            <Close />
        </DialogClose>
    </RadixDialog.Overlay>
));
DialogOverlay.displayName = RadixDialog.Overlay.displayName;

const DialogContent = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Content>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Content> & {
        containerId?: string;
        showCloseOnOverlay?: boolean;
        position?: DialogPosition;
    }
>(
    (
        {
            className,
            containerId,
            showCloseOnOverlay,
            children,
            position = DialogPosition.Center,
            ...props
        },
        ref,
    ) => {
        const [containerElement, setContainerElement] = useState<HTMLElement | undefined>(
            undefined,
        );

        useEffect(() => {
            // This ensures document.getElementById is called in the client-side environment only.
            // note. containerElement cant be null
            const element = containerId ? document.getElementById(containerId) : undefined;
            setContainerElement(element ?? undefined);
        }, [containerId]);
        const positionClass =
            position === DialogPosition.Right
                ? 'right-0 h-screen top-0 w-full max-w-[500px]'
                : 'max-h-[60vh] left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-xl w-80 max-w-[85vw]';
        return (
            <RadixDialog.Portal container={containerElement}>
                <DialogOverlay showCloseIcon={showCloseOnOverlay} />
                <RadixDialog.Content
                    ref={ref}
                    className={cx(
                        'fixed z-[99999] flex flex-col justify-center overflow-hidden bg-primary-100 dark:bg-neutral-6 md:w-96',
                        positionClass,
                    )}
                    {...props}
                >
                    <VisuallyHidden.Root>
                        <RadixDialog.Title />
                        <RadixDialog.Description />
                    </VisuallyHidden.Root>
                    {children}
                </RadixDialog.Content>
            </RadixDialog.Portal>
        );
    },
);
DialogContent.displayName = RadixDialog.Content.displayName;

const DialogTitle = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Title>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Title>
>((props, ref) => (
    <RadixDialog.Title
        ref={ref}
        className="font-inter text-title-lg text-neutral-10 dark:text-neutral-92"
        {...props}
    />
));
DialogTitle.displayName = RadixDialog.Title.displayName;

const DialogBody = React.forwardRef<React.ElementRef<'div'>, React.ComponentPropsWithoutRef<'div'>>(
    (props, ref) => (
        <div
            ref={ref}
            className="p-md--rs text-body-sm text-neutral-40 dark:text-neutral-60"
            {...props}
        />
    ),
);
DialogBody.displayName = 'DialogBody';

export { Dialog, DialogClose, DialogTrigger, DialogContent, DialogTitle, DialogBody };
