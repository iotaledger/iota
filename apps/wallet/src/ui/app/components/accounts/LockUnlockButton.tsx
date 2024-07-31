// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LockLocked16, LockUnlocked16 } from '@iota/icons';
import { type ComponentPropsWithoutRef } from 'react';

import { Tooltip } from '../../shared/tooltip';
import LoadingIndicator from '../loading/LoadingIndicator';

interface LockUnlockButtonProps extends ComponentPropsWithoutRef<'button'> {
    isLocked: boolean;
    isLoading: boolean;
}

export function LockUnlockButton({ isLocked, onClick, isLoading }: LockUnlockButtonProps) {
    return (
        <Tooltip tip={isLocked ? 'Unlock Account' : 'Lock Account'}>
            <button
                className="text-steel hover:text-hero-dark ml-auto flex cursor-pointer appearance-none items-center justify-center border-none bg-transparent p-0"
                onClick={onClick}
                data-testid={isLocked ? 'unlock-account-button' : 'lock-account-button'}
            >
                {isLoading ? (
                    <LoadingIndicator />
                ) : isLocked ? (
                    <LockLocked16 className="h-4 w-4" />
                ) : (
                    <LockUnlocked16 className="h-4 w-4" />
                )}
            </button>
        </Tooltip>
    );
}
