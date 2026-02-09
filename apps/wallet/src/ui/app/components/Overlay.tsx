// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';
import type { ReactNode } from 'react';
import { Header } from '@iota/apps-ui-kit';
import { Portal } from '../shared/Portal';
import { useNavigate } from 'react-router-dom';

interface OverlayProps {
    title?: string;
    children: ReactNode;
    showModal: boolean;
    closeOverlay?: () => void;
    closeIcon?: ReactNode | null;
    setShowModal?: (showModal: boolean) => void;
    background?: 'bg-iota-neutral-100 dark:bg-iota-neutral-6';
    titleCentered?: boolean;
    showBackButton?: boolean;
    onBack?: () => void;
    hideCloseIcon?: boolean;
    headerAction?: ReactNode;
}

export function Overlay({
    title,
    children,
    showModal,
    closeOverlay,
    setShowModal,
    titleCentered = true,
    showBackButton,
    onBack,
    headerAction,
    hideCloseIcon,
}: OverlayProps) {
    const closeModal = useCallback(
        (e: React.MouseEvent<HTMLElement>) => {
            closeOverlay && closeOverlay();
            setShowModal && setShowModal(false);
        },
        [closeOverlay, setShowModal],
    );
    const navigate = useNavigate();
    const handleBack = useCallback(() => {
        if (onBack) {
            onBack();
        } else {
            navigate(-1);
        }
    }, [onBack, navigate]);
    return showModal ? (
        <Portal containerId="overlay-portal-container">
            <div className="absolute inset-0 z-[9999] flex flex-col flex-nowrap items-center backdrop-blur-[20px]">
                {title && (
                    <div className="relative w-full">
                        <Header
                            onBack={showBackButton ? handleBack : undefined}
                            title={title}
                            onClose={!hideCloseIcon ? closeModal : undefined}
                            titleCentered={titleCentered}
                            testId="overlay-title"
                        />
                        {headerAction && hideCloseIcon && (
                            <div className="absolute right-4 top-[-4.5px] translate-y-1/2">
                                {headerAction}
                            </div>
                        )}
                    </div>
                )}
                <div className="flex w-full flex-1 flex-col overflow-hidden bg-iota-neutral-100 p-md dark:bg-iota-neutral-6">
                    {children}
                </div>
            </div>
        </Portal>
    ) : null;
}
