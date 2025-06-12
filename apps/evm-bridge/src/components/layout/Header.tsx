import { Close, MenuIcon } from '@iota/apps-ui-icons';
import { IOTABridgeLogo, ThemeSwitcher } from '..';
import { ConnectButton } from '@iota/dapp-kit';
import { ConnectButtonL2 } from './connect-buttons';
import { useState } from 'react';
import { Button, ButtonType, Divider } from '@iota/apps-ui-kit';
import clsx from 'clsx';

export function Header(): React.JSX.Element {
    const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);

    const MobileIcon = isMobileMenuOpen ? Close : MenuIcon;
    return (
        <div className="fixed top-0 left-0 py-md--rs backdrop-blur-lg z-10 w-full">
            <div className="container flex justify-between items-center">
                <IOTABridgeLogo className="dark:text-neutral-92 text-neutral-10" />
                <div className="flex flex-row gap-xs">
                    <ThemeSwitcher />

                    <div className="hidden md:flex flex-row gap-xs items-center">
                        <ConnectButton
                            data-testid="connect-l1-wallet"
                            className="text-label-lg h-10"
                            connectText="Connect L1 Wallet"
                            size="md"
                        />
                        <ConnectButtonL2 />
                    </div>

                    <div className="flex md:hidden">
                        <Button
                            type={ButtonType.Ghost}
                            icon={<MobileIcon className="h-5 w-5" />}
                            onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
                        />
                    </div>
                </div>
            </div>
            <div
                className={clsx(
                    isMobileMenuOpen ? 'max-h-[300px]' : 'max-h-0',
                    'transition-[max-height] overflow-y-hidden md:hidden',
                )}
            >
                <div className="flex flex-col gap-md items-end p-lg">
                    <ConnectButton
                        data-testid="connect-l1-wallet"
                        className="text-label-lg h-10"
                        connectText="Connect L1 Wallet"
                        size="md"
                    />
                    <Divider />
                    <ConnectButtonL2 />
                </div>
            </div>
        </div>
    );
}
