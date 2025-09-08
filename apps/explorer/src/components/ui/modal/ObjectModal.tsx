// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Modal, type ModalProps } from './Modal';
import { Close } from '@iota/apps-ui-icons';
import { resolveNFTMedia, useNFTMediaHeaders } from '@iota/core';
import { LoadingIndicator, MediaFallback, AssetMediaRenderer } from '@iota/apps-ui-kit';

export interface ObjectModalProps extends Omit<ModalProps, 'children'> {
    title: string;
    subtitle: string;
    alt: string;
    src: string;
}

export function ObjectModal({
    open,
    onClose,
    alt,
    title,
    subtitle,
    src,
}: ObjectModalProps): JSX.Element {
    const { data: nftMediaHeaders, isLoading } = useNFTMediaHeaders(src);
    const { type, shouldAutoPlayVideo, isMediaSupported } = resolveNFTMedia(src, nftMediaHeaders);

    return (
        <Modal open={open} onClose={onClose}>
            <div className="flex h-full w-full flex-col gap-5">
                {isLoading ? (
                    <LoadingIndicator />
                ) : !isMediaSupported ? (
                    <MediaFallback />
                ) : (
                    <AssetMediaRenderer
                        assetType={type}
                        src={src}
                        isAutoPlayEnabled={shouldAutoPlayVideo}
                    />
                )}
                <div className="flex flex-col gap-3">
                    <span className="truncate text-headline-md text-iota-neutral-100">{title}</span>
                    <span className="text-label-lg text-iota-neutral-90">{subtitle}</span>
                </div>
            </div>
            <div className="absolute -right-12 top-0 inline-flex h-8 w-8 cursor-pointer items-center justify-center rounded-full  bg-shader-inverted-dark-16 p-xs text-iota-neutral-100 outline-none hover:text-iota-neutral-92">
                <Close onClick={onClose} className="h-5 w-5" aria-label="Close" />
            </div>
        </Modal>
    );
}
