// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { PropsWithChildren } from 'react';
import { DialogBody, DialogContent, DialogPosition } from '@iota/apps-ui-kit';

export function Layout({ children }: PropsWithChildren) {
    return (
        <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
            <div className="flex h-full flex-col">{children}</div>
        </DialogContent>
    );
}

export function LayoutBody({ children }: PropsWithChildren) {
    return (
        <div className="flex w-full flex-1 overflow-y-hidden [&_>div]:w-full [&_>div]:overflow-y-auto">
            <DialogBody>{children}</DialogBody>
        </div>
    );
}

export function LayoutFooter({ children }: PropsWithChildren) {
    return <div className="p-md--rs">{children}</div>;
}
