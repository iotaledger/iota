// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { X32 } from '@iota/icons';
import cl from 'clsx';
import { useCallback } from 'react';
import type { ReactNode } from 'react';

import useAppSelector from '../../hooks/useAppSelector';
import { AppType } from '../../redux/slices/app/AppType';
import { Portal } from '../../shared/Portal';
import st from './Overlay.module.scss';

type OverlayProps = {
    title?: ReactNode;
    children: ReactNode;
    showModal: boolean;
    closeOverlay?: () => void;
    closeIcon?: ReactNode | null;
    setShowModal?: (showModal: boolean) => void;
    background?: 'bg-iota-lightest';
};

function Overlay({
    title,
    children,
    showModal,
    closeOverlay,
    setShowModal,
    closeIcon = <X32 fill="currentColor" className="h-8 w-8 text-iota-light" />,
    background,
}: OverlayProps) {
    const closeModal = useCallback(
        (e: React.MouseEvent<HTMLElement>) => {
            closeOverlay && closeOverlay();
            setShowModal && setShowModal(false);
        },
        [closeOverlay, setShowModal],
    );
    const appType = useAppSelector((state) => state.app.appType);
    const isFullScreen = appType === AppType.fullscreen;

    return showModal ? (
        <Portal containerId="overlay-portal-container">
            <div
                className={cl(st.container, {
                    [st.fullScreenContainer]: isFullScreen,
                })}
            >
                {title && (
                    <div className="h-12 w-full bg-gray-40">
                        <div
                            data-testid="overlay-title"
                            className="flex h-12 items-center justify-center bg-gray-40 text-heading4 font-semibold text-steel-darker"
                        >
                            {title}
                        </div>
                    </div>
                )}
                <div
                    className={cl(st.content, background)}
                    style={{
                        height: title ? 'calc(100% - 128px)' : 'calc(100% - 80px)',
                    }}
                >
                    {children}
                </div>
                <button data-testid="close-icon" className={st.closeOverlay} onClick={closeModal}>
                    {closeIcon}
                </button>
            </div>
        </Portal>
    ) : null;
}

export default Overlay;
