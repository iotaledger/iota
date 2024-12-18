// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '../../shared/ButtonUI';
import { Heading } from '../../shared/heading';
import { Portal } from '../../shared/Portal';
import WarningSvg from './warning.svg';

export type ScamOverlayProps = {
    onDismiss(): void;
    open: boolean;
    title?: string;
    subtitle?: string;
};

export function ScamOverlay({ open, onDismiss, title, subtitle }: ScamOverlayProps) {
    if (!open) return null;
    return (
        <Portal containerId="overlay-portal-container">
            <div className="absolute bottom-0 left-0 top-0 z-50 flex h-full w-full flex-col items-center justify-center gap-4 bg-issue-light p-4">
                <WarningSvg />
                <div className="flex flex-col gap-2 pb-4 text-center">
                    <Heading variant="heading2" weight="semibold" color="gray-90">
                        {title || 'Malicious website'}
                    </Heading>
                    <div className="flex text-center text-pBody font-medium text-gray-90">
                        <div className="text-pBody font-medium text-gray-90">
                            {subtitle ||
                                'This website has been flagged for malicious behavior. To protect your wallet from potential threats, please return to safety.'}
                        </div>
                    </div>
                </div>

                <div className="mt-auto w-full items-stretch gap-2">
                    <Button variant="primary" text="I understand" onClick={onDismiss} />
                </div>
            </div>
        </Portal>
    );
}
