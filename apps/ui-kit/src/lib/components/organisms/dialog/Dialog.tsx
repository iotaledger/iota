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
        className={cx(
            'dialog-overlay-bg absolute h-full w-full backdrop-blur-md names:backdrop-blur-lg',
            'data-[state=open]:animate-dialog-overlay-show',
        )}
        {...props}
    >
        <DialogClose
            className={cx('fixed right-3 top-3', { hidden: !showCloseIcon })}
            aria-label="Close"
        >
            <Close className="button-text-color-neutral" />
        </DialogClose>
    </RadixDialog.Overlay>
));
DialogOverlay.displayName = RadixDialog.Overlay.displayName;

const DialogContainer = React.forwardRef<
    HTMLDivElement,
    React.PropsWithChildren<{ isFixedPosition: boolean }>
>((props, ref) => (
    <div
        className={cx('inset-0 z-[99999]', props.isFixedPosition ? 'fixed' : 'absolute')}
        ref={ref}
    >
        <div className="relative h-full w-full">{props.children}</div>
    </div>
));

const DialogContent = React.forwardRef<
    React.ElementRef<typeof RadixDialog.Content>,
    React.ComponentPropsWithoutRef<typeof RadixDialog.Content> & {
        containerId?: string;
        showCloseOnOverlay?: boolean;
        position?: DialogPosition;
        customWidth?: string;
        isFixedPosition?: boolean;
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
            isFixedPosition = position === DialogPosition.Right,
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
        const dialogPositioning =
            position === DialogPosition.Right
                ? 'overflow-hidden right-0 h-screen top-0 w-full'
                : 'overflow-y-auto overflow-x-hidden left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-xl';
        const widthClass =
            position === DialogPosition.Right ? 'md:w-96 max-w-[500px]' : customWidth;
        const heightClass = position === DialogPosition.Right ? 'h-screen' : 'max-h-[90vh] h-full';
        return (
            <RadixDialog.Portal container={containerElement}>
                <DialogContainer isFixedPosition={isFixedPosition}>
                    <DialogOverlay showCloseIcon={showCloseOnOverlay} position={position} />
                    <RadixDialog.Content
                        ref={ref}
                        className={cx(
                            'dialog-content-bg dialog-outline absolute flex flex-col justify-center',
                            dialogPositioning,
                            widthClass,
                            position === DialogPosition.Right
                                ? 'data-[state=open]:animate-dialog-panel-show'
                                : 'data-[state=open]:animate-dialog-content-show',
                        )}
                        {...props}
                    >
                        <VisuallyHidden.Root>
                            <RadixDialog.Title />
                            <RadixDialog.Description />
                        </VisuallyHidden.Root>
                        <div className={cx('flex flex-1 flex-col', heightClass)}>{children}</div>
                    </RadixDialog.Content>
                </DialogContainer>
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
        className="dialog-title-color font-inter text-title-lg"
        {...props}
    />
));
DialogTitle.displayName = RadixDialog.Title.displayName;

const DialogBody = React.forwardRef<React.ElementRef<'div'>, React.ComponentPropsWithoutRef<'div'>>(
    (props, ref) => (
        <div
            ref={ref}
            className="dialog-body-color flex-1 overflow-y-auto p-md--rs text-body-sm"
            {...props}
        />
    ),
);
DialogBody.displayName = 'DialogBody';

export { Dialog, DialogClose, DialogTrigger, DialogContent, DialogTitle, DialogBody };
