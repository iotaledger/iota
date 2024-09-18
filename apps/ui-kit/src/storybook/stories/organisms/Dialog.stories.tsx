// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ButtonSize } from '@/lib/components';

import type { Meta, StoryObj } from '@storybook/react';

import {
    ButtonType,
    DialogContent,
    DialogBody,
    Button,
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    Dialog,
    Divider,
    Header,
    ImageShape,
    ImageType,
} from '@/components';
import { useState } from 'react';

const meta = {
    component: Dialog,
    tags: ['autodocs'],
    render: () => {
        const [open, setOpen] = useState(false);
        return (
            <div className="flex">
                <Button size={ButtonSize.Small} text="Open Dialog" onClick={() => setOpen(true)} />
                <Dialog open={open} onOpenChange={setOpen}>
                    <DialogContent showCloseOnOverlay>
                        <Header
                            title="Connect Ledger Wallet"
                            titleCentered
                            onClose={() => setOpen(false)}
                            onBack={() => setOpen(false)}
                        />
                        <DialogBody>
                            <div className="flex flex-col items-center gap-2">
                                <div className="mt-4.5">Logo</div>
                                <div className="mt-4.5 break-words text-center">
                                    Connect your ledger to your computer, unlock it, and launch the
                                    IOTA app. Click Continue when done.
                                </div>
                                <div className="mt-4.5"> Need more help? View tutorial.</div>
                            </div>
                        </DialogBody>
                        <div className="flex w-full flex-row justify-center gap-2 px-md--rs pb-md--rs pt-sm--rs">
                            <Button
                                size={ButtonSize.Small}
                                type={ButtonType.Outlined}
                                text="Cancel"
                                onClick={() => setOpen(false)}
                            />
                            <Button size={ButtonSize.Small} text="Connect" />
                        </div>
                    </DialogContent>
                </Dialog>
            </div>
        );
    },
} satisfies Meta<typeof Dialog>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren, ReactNode } from 'react';
import { ArrowTopRight, Exclamation } from '@iota/ui-icons';
enum ModalFooterContent {
    GettingStarted = 'getting-started',
    ConnectionStatus = 'connection-status',
}

const DEFAULT_FOOTER_CONTENT = ModalFooterContent.ConnectionStatus;

interface ConnectModalProps {
    isModalOpen?: boolean;
}

function ConnectModal({ isModalOpen }: ConnectModalProps) {
    const [activeFooterContent, setActiveFooterContent] =
        useState<ModalFooterContent>(DEFAULT_FOOTER_CONTENT);

    const resetSelection = () => {
        setActiveFooterContent(DEFAULT_FOOTER_CONTENT);
    };

    const handleOpenChange = (open: boolean) => {
        if (!open) {
            resetSelection();
        }
    };

    const footerContentMap: Record<ModalFooterContent, () => ReactNode> = {
        [ModalFooterContent.GettingStarted]: () => <GettingStarted />,
        [ModalFooterContent.ConnectionStatus]: () => <ConnectionStatus hadConnectionError={true} />,
    };

    const FooterContent = footerContentMap[activeFooterContent];

    return (
        <Dialog open={isModalOpen} onOpenChange={handleOpenChange}>
            <DialogContent showCloseOnOverlay>
                <Header
                    title="Connect a Wallet"
                    onClose={() => handleOpenChange(false)}
                    onBack={() =>
                        setActiveFooterContent(
                            activeFooterContent === ModalFooterContent.ConnectionStatus
                                ? ModalFooterContent.GettingStarted
                                : ModalFooterContent.ConnectionStatus,
                        )
                    }
                />
                <Divider />
                <div className="flex flex-col items-center p-xs--rs">
                    <WalletList
                        onPlaceholderClick={() =>
                            setActiveFooterContent(ModalFooterContent.GettingStarted)
                        }
                    />
                </div>
                <Divider />
                <ModalFooter>
                    <FooterContent />
                </ModalFooter>
            </DialogContent>
        </Dialog>
    );
}

function ModalFooter({ children }: PropsWithChildren) {
    return (
        <div className="flex w-full flex-col justify-center px-md--rs py-sm--rs">{children}</div>
    );
}

type WalletListProps = {
    onPlaceholderClick: () => void;
};

const IotaIcon = () => (
    <svg width={28} height={28} fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect width={28} height={28} rx={6} fill="#6FBCF0" />
        <path
            fillRule="evenodd"
            clipRule="evenodd"
            d="M7.942 20.527A6.875 6.875 0 0 0 13.957 24c2.51 0 4.759-1.298 6.015-3.473a6.875 6.875 0 0 0 0-6.945l-5.29-9.164a.837.837 0 0 0-1.45 0l-5.29 9.164a6.875 6.875 0 0 0 0 6.945Zm4.524-11.75 1.128-1.953a.418.418 0 0 1 .725 0l4.34 7.516a5.365 5.365 0 0 1 .449 4.442 4.675 4.675 0 0 0-.223-.73c-.599-1.512-1.954-2.68-4.029-3.47-1.426-.54-2.336-1.336-2.706-2.364-.476-1.326.021-2.77.316-3.44Zm-1.923 3.332L9.255 14.34a5.373 5.373 0 0 0 0 5.43 5.373 5.373 0 0 0 4.702 2.714 5.38 5.38 0 0 0 3.472-1.247c.125-.314.51-1.462.034-2.646-.44-1.093-1.5-1.965-3.15-2.594-1.864-.707-3.076-1.811-3.6-3.28a4.601 4.601 0 0 1-.17-.608Z"
            fill="#fff"
        />
    </svg>
);

function WalletList({ onPlaceholderClick }: WalletListProps) {
    return (
        <>
            <WalletListItem
                name="Iota Wallet"
                icon={<IotaIcon />}
                onClick={onPlaceholderClick}
                isSelected
            />
        </>
    );
}

interface WalletListItemProps {
    name: string;
    icon: ReactNode;
    isSelected?: boolean;
    onClick: () => void;
}

function WalletListItem({ name, icon, isSelected, onClick }: WalletListItemProps) {
    return (
        <Card type={!isSelected ? CardType.Default : CardType.Outlined} onClick={onClick}>
            <CardImage type={ImageType.Placeholder} shape={ImageShape.SquareRounded}>
                {typeof icon === 'string' ? <img src={icon} alt={`${name} logo`} /> : icon}
            </CardImage>
            <CardBody title={name} />
            <CardAction type={CardActionType.Link} />
        </Card>
    );
}

function GettingStarted() {
    return (
        <div className="flex flex-row items-center justify-between gap-md">
            <p className="text-body-md text-neutral-10 dark:text-neutral-92">
                Don't have a wallet yet?
            </p>
            <a href={''} target="_blank" rel="noopener noreferrer">
                <Button text="Install it here" icon={<ArrowTopRight />} type={ButtonType.Primary} />
            </a>
        </div>
    );
}

interface ConnectionStatusProps {
    hadConnectionError: boolean;
}

function ConnectionStatus({ hadConnectionError }: ConnectionStatusProps) {
    return (
        <div className="flex flex-row items-center gap-x-md">
            {!hadConnectionError && <IotaIcon />}
            <div
                className={`flex items-start gap-y-xxxs ${hadConnectionError ? 'w-full flex-row' : 'flex-col'}`}
            >
                {hadConnectionError ? (
                    <div className="flex w-full flex-row items-center justify-between gap-sm">
                        <p className="flex flex-row items-center gap-xxs text-body-md text-error-40 dark:text-error-60">
                            <Exclamation className="h-4 w-4" /> Connection failed
                        </p>
                        <Button text="Retry Connection" onClick={() => {}} />
                    </div>
                ) : (
                    <>
                        <p className="text-body-lg text-neutral-10 dark:text-neutral-92">
                            Opening Hola
                        </p>
                        <p className="dark:text-neutral-62 text-body-md text-neutral-40">
                            Confirm connection in the wallet...
                        </p>
                    </>
                )}
            </div>
        </div>
    );
}

export const ConnectWallet: Story = {
    render: () => <ConnectModal isModalOpen />,
};
