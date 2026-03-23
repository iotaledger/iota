// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useEffect } from 'react';
import { Button, ButtonType, Dialog, DialogContent, DialogPosition } from '@iota/apps-ui-kit';
import { Close } from '@iota/apps-ui-icons';
import Image from 'next/image';

export type InterstitialConfig = {
    enabled: boolean;
    dismissKey?: string;
    imageUrl?: string;
    bannerUrl?: string;
};

type InterstitialProps = InterstitialConfig & {
    onClose: () => void;
};

const getValidDismissKey = (dismissKey?: string) => {
    const normalized = dismissKey?.trim();
    return normalized && normalized !== 'undefined' ? normalized : null;
};

const setInterstitialDismissed = (dismissKey: string) => localStorage.setItem(dismissKey, 'true');

export function Interstitial({ dismissKey, imageUrl, bannerUrl, onClose }: InterstitialProps) {
    const validDismissKey = getValidDismissKey(dismissKey);

    useEffect(() => {
        if (!validDismissKey) return;
        const t = setTimeout(() => setInterstitialDismissed(validDismissKey), 1000);
        return () => clearTimeout(t);
    }, [validDismissKey]);

    const close = () => {
        if (validDismissKey) {
            setInterstitialDismissed(validDismissKey);
        }
        onClose();
    };

    if (!imageUrl) return null;

    const imageNode = (
        <Image
            src={imageUrl}
            alt="interstitial banner image"
            width={308}
            height={616}
            style={{ height: '80vh', width: 'auto' }}
            unoptimized
        />
    );

    return (
        <Dialog open onOpenChange={(open) => !open && close()}>
            <DialogContent
                position={DialogPosition.Center}
                customWidth="w-auto"
                showCloseOnOverlay={false}
            >
                <div className="relative h-full overflow-hidden rounded-3xl">
                    {bannerUrl ? (
                        <a
                            href={bannerUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            onClick={close}
                            className="block h-full"
                        >
                            {imageNode}
                        </a>
                    ) : (
                        imageNode
                    )}

                    <div className="absolute right-3 top-3">
                        <Button
                            type={ButtonType.Secondary}
                            onClick={close}
                            icon={<Close className="h-4 w-4" />}
                            aria-label="Close"
                        />
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
