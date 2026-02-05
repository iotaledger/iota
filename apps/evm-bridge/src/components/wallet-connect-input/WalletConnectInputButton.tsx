// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Link as LinkIcon } from '@iota/apps-ui-icons';
import { ConnectModal } from '@iota/dapp-kit';
import { useConnectModal } from '@rainbow-me/rainbowkit';
import clsx from 'clsx';
import { type ComponentPropsWithoutRef, forwardRef } from 'react';

interface ConnectInputProps {
    label: string;
    isDestination?: boolean;
    isLayer1: boolean;
}

export function WalletConnectInput({ label, isDestination, isLayer1 }: ConnectInputProps) {
    const { openConnectModal } = useConnectModal();

    return (
        <div
            className={clsx(
                'flex items-start w-full gap-y-xs',
                isDestination ? 'flex-col-reverse' : 'flex-col',
            )}
        >
            <span className="text-label-lg text-iota-neutral-40 dark:text-iota-neutral-60">
                {label}
            </span>

            {!!isLayer1 && <ConnectModal trigger={<WalletConnectionInputButton />} />}
            {!isLayer1 && <WalletConnectionInputButton onClick={openConnectModal} />}
        </div>
    );
}
export const WalletConnectionInputButton = forwardRef<
    HTMLButtonElement,
    ComponentPropsWithoutRef<'button'>
>(function WalletConnectInputButton(props, ref) {
    return (
        <button
            type="button"
            ref={ref}
            {...props}
            className="group w-full px-md py-sm rounded-lg border border-iota-neutral-80 hover:border-iota-primary-60 dark:hover:border-iota-primary-80 dark:border-iota-neutral-60 focus:border-iota-primary-30 focus:dark:border-iota-primary-80"
        >
            <div className="flex flex-row items-center justify-between gap-x-sm w-full group-hover:opacity-80 dark:text-iota-neutral-92 text-iota-neutral-12 dark:group-hover:text-iota-primary-80 group-hover:text-iota-primary-40">
                <span className="text-start text-title-md leading-6">Connect Wallet</span>
                <LinkIcon className="h-6 w-6 " />
            </div>
        </button>
    );
});
