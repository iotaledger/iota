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
            <span className="text-label-lg text-neutral-40 dark:text-neutral-60">{label}</span>

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
            className="group w-full px-md py-sm rounded-lg border border-neutral-80 hover:border-primary-60 dark:hover:border-primary-80 dark:border-neutral-60 focus:border-primary-30 focus:dark:border-primary-80"
        >
            <div className="flex flex-row items-center justify-between gap-x-sm w-full group-hover:opacity-80 dark:text-neutral-92 text-neutral-12 dark:group-hover:text-primary-80 group-hover:text-primary-40">
                <span className="text-start text-title-md leading-6">Connect Wallet</span>
                <LinkIcon className="h-6 w-6 " />
            </div>
        </button>
    );
});
