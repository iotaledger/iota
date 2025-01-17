// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Pined, Unpined } from '@iota/apps-ui-icons';

interface PinButtonProps {
    isPinned?: boolean;
    onClick: () => void;
}

export function PinButton({ isPinned, onClick }: PinButtonProps) {
    return (
        <button
            type="button"
            className="cursor-pointer border-none bg-transparent [&_svg]:h-4 [&_svg]:w-4"
            aria-label={isPinned ? 'Unpin Coin' : 'Pin Coin'}
            onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                onClick();
            }}
        >
            {isPinned ? (
                <Pined className="text-primary-40" />
            ) : (
                <Unpined className="text-neutral-60" />
            )}
        </button>
    );
}
