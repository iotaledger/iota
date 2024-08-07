// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header } from '@iota/apps-ui-kit';
import { useCallback } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';

interface HeaderTemplateProps {
    title?: string;
    children: ReactNode;
    closeHeaderTemplate?: () => void;
    setShowModal?: (showModal: boolean) => void;
    isTitleCentered?: boolean;
    displayBackButton?: boolean;
}

function HeaderTemplate({
    title,
    children,
    closeHeaderTemplate,
    setShowModal,
    isTitleCentered,
    displayBackButton,
}: HeaderTemplateProps) {
    const closeModal = useCallback(() => {
        closeHeaderTemplate && closeHeaderTemplate();
        setShowModal && setShowModal(false);
    }, [closeHeaderTemplate, setShowModal]);
    const navigate = useNavigate();

    return (
        <div className="h-full w-full">
            {title && (
                <Header
                    titleCentered={isTitleCentered}
                    title={title}
                    onBack={displayBackButton ? () => navigate(-1) : undefined}
                    onClose={closeModal}
                />
            )}
            <div className="h-full w-full bg-white p-md">{children}</div>
        </div>
    );
}

export default HeaderTemplate;
