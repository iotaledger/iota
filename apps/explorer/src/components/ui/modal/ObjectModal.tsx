// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Modal, type ModalProps } from './Modal';
import { Image } from '../image/Image';
import { Close } from '@iota/apps-ui-icons';

export interface ObjectModalProps extends Omit<ModalProps, 'children'> {
    title: string;
    subtitle: string;
    alt: string;
    src: string;
    video?: string | null;
    moderate?: boolean;
}

export function ObjectModal({
    open,
    onClose,
    alt,
    title,
    subtitle,
    src,
    video,
    // NOTE: Leave false only if ObjectModal is paired with an Image component
    moderate = false,
}: ObjectModalProps): JSX.Element {
    return (
        <Modal open={open} onClose={onClose}>
            <div className="flex flex-col gap-5">
                {video ? (
                    <video controls className="h-full w-full" src={video} />
                ) : (
                    // Moderation is disabled inside the modal so if a user clicks to open an unblurred image the experience is consistent
                    <Image alt={alt} src={src} rounded="none" moderate={moderate} />
                )}
                <div className="flex flex-col gap-3">
                    <span className="text-headline-md text-neutral-100">{title}</span>
                    <span className="text-label-lg text-neutral-90">{subtitle}</span>
                </div>
            </div>
            <div className="absolute -right-12 top-0 inline-flex h-8 w-8 cursor-pointer items-center justify-center rounded-full  bg-shader-inverted-dark-16 p-xs text-neutral-100 outline-none hover:text-neutral-92">
                <Close onClick={onClose} className="h-5 w-5" aria-label="Close" />
            </div>
        </Modal>
    );
}
