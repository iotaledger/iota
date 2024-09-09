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
    background?: 'bg-iota-lightest';
    titleCentered?: boolean;
    showBackButton?: boolean;
    onBack?: () => void;
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
                    <Header
                        onBack={showBackButton ? handleBack : undefined}
                        title={title}
                        onClose={closeModal}
                        titleCentered={titleCentered}
                    />
                )}
                <div className="flex w-full flex-1 flex-col overflow-hidden bg-neutral-100 p-md">
                    {children}
                </div>
            </div>
        </Portal>
    ) : null;
}
