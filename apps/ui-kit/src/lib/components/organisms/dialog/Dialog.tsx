// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as RadixDialog from '@radix-ui/react-dialog';
import * as VisuallyHidden from '@radix-ui/react-visually-hidden';
import cx from 'classnames';
import * as React from 'react';
import { Close } from '@iota/apps-ui-icons';
import { useEffect, useState } from 'react';
import { DialogPosition } from './dialog.enums';

const Dialog = ({
    open,
    onOpenChange,
    children,
    ...props
}: React.ComponentPropsWithoutRef<typeof RadixDialog.Root>) => {
    const handleOpenChange = (isOpen: boolean) => {
        const isMouseOverToast = document.querySelector('.toast-layer:hover');
        if (!isOpen && isMouseOverToast) {
            return;
        }
        onOpenChange?.(isOpen);
    };

    return (
        <RadixDialog.Root open={open} onOpenChange={handleOpenChange} {...props}>
            {children}
        </RadixDialog.Root>
    );
};
const DialogTrigger = RadixDialog.Trigger;
const DialogClose = RadixDialog.Close;

const DialogOverlay = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Overlay>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Overlay> & {
        showCloseIcon?: boolean;
        position?: DialogPosition;
    }
>(({ showCloseIcon, position, ...props }, ref) => (
    <RadixDialog.Overlay
        ref={ref}
        className={cx('inset-0 z-[99998] bg-shader-neutral-light-48 backdrop-blur-md', {
            fixed: position === DialogPosition.Right,
            absolute: position === DialogPosition.Center,
        })}
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
        customWidth?: string;
    }
>(
    (
        {
            className,
            containerId,
            showCloseOnOverlay,
            children,
            position = DialogPosition.Center,
            customWidth = 'w-80 max-w-[85vw] md:w-96',
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
                ? 'overflow-hidden right-0 h-screen top-0 w-full'
                : 'overflow-y-auto overflow-x-hidden left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-xl';
        const widthClass =
            position === DialogPosition.Right ? 'md:w-96 max-w-[500px]' : customWidth;
        const heightClass = position === DialogPosition.Right ? 'h-screen' : 'max-h-[80vh] h-full';
        return (
            <RadixDialog.Portal container={containerElement}>
                <DialogOverlay showCloseIcon={showCloseOnOverlay} position={position} />
                <RadixDialog.Content
                    ref={ref}
                    className={cx(
                        'fixed z-[99999] flex flex-col justify-center bg-primary-100 dark:bg-neutral-6',
                        positionClass,
                        widthClass,
                    )}
                    {...props}
                >
                    <VisuallyHidden.Root>
                        <RadixDialog.Title />
                        <RadixDialog.Description />
                    </VisuallyHidden.Root>
                    <div className={cx('flex flex-1 flex-col', heightClass)}>{children}</div>
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
            className="flex-1 overflow-y-auto p-md--rs text-body-sm text-neutral-40 dark:text-neutral-60"
            {...props}
        />
    ),
);
DialogBody.displayName = 'DialogBody';

export { Dialog, DialogClose, DialogTrigger, DialogContent, DialogTitle, DialogBody };
