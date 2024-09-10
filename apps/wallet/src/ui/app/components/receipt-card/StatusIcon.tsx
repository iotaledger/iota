// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ThumbUpFill32 } from '@iota/icons';
import cl from 'clsx';

interface StatusIconProps {
    status: boolean;
}

export function StatusIcon({ status }: StatusIconProps) {
    return (
        <div
            className={cl(
                'flex h-12 w-12 items-center  justify-center rounded-full border-2 border-dotted p-1',
                status ? 'border-success' : 'border-issue',
            )}
        >
            <div
                className={cl(
                    'flex h-8 w-8 items-center justify-center rounded-full',
                    status ? 'bg-success' : 'bg-issue',
                )}
            >
                <ThumbUpFill32
                    fill="currentColor"
                    className={cl('text-2xl text-white', !status && 'rotate-180')}
                />
            </div>
        </div>
    );
}
