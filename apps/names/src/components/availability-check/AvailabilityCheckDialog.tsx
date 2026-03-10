// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Close } from '@iota/apps-ui-icons';
import { Dialog, DialogClose, DialogContent } from '@iota/apps-ui-kit';

import { useAvailabilityCheckDialog } from '@/stores/useAvailabilityCheckDialog';

import { AvailabilityCheck } from './AvailabilityCheck';

export function AvailabilityCheckDialog() {
    const { isOpen, close, props } = useAvailabilityCheckDialog();

    return (
        <Dialog open={isOpen} onOpenChange={() => close()}>
            <DialogContent
                isFixedPosition
                showCloseOnOverlay={false}
                customWidth="w-[90vw] h-[clamp(400px,90vh,700px)] lg:w-[60vw] lg:h-[clamp(400px,80vh,700px)]"
            >
                <div className="flex flex-col gap-md px-lg py-10 md:px-24 md:py-20 lg:px-48 flex-1">
                    <DialogClose className="absolute right-4 top-4 xs:right-6 xs:top-6">
                        <Close className="w-6 h-6 button-text-color-neutral" />
                    </DialogClose>

                    <AvailabilityCheck {...props} />
                </div>
            </DialogContent>
        </Dialog>
    );
}
