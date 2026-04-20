// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header } from '@iota/apps-ui-kit';
import cn from 'clsx';
import { useCallback, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';

interface PageTemplateProps {
    title?: string;
    children: ReactNode;
    onClose?: () => void;
    isTitleCentered?: boolean;
    showBackButton?: boolean;
    onBack?: () => void;
    headerAction?: ReactNode;
}

export function PageTemplate({
    title,
    children,
    onClose,
    isTitleCentered,
    showBackButton,
    onBack,
    headerAction,
}: PageTemplateProps) {
    const navigate = useNavigate();
    const [isScrolled, setIsScrolled] = useState(false);
    const scrollRef = useRef<HTMLDivElement>(null);

    const handleBack = useCallback(() => {
        if (onBack) {
            onBack();
        } else {
            navigate(-1);
        }
    }, [navigate, onBack]);

    const handleScroll = useCallback(() => {
        setIsScrolled((scrollRef.current?.scrollTop ?? 0) > 2);
    }, []);

    return (
        <div className="flex h-full w-full flex-col">
            {title && (
                <div
                    className={cn(
                        'relative border-b transition-colors duration-200',
                        isScrolled
                            ? 'border-shader-neutral-light-8 dark:border-shader-neutral-dark-8'
                            : 'border-transparent',
                    )}
                >
                    <Header
                        titleCentered={isTitleCentered}
                        title={title}
                        onBack={showBackButton ? handleBack : undefined}
                        onClose={onClose}
                    />
                    {headerAction && !onClose && (
                        <div className="absolute right-4 top-1/2 -translate-y-1/2">
                            {headerAction}
                        </div>
                    )}
                </div>
            )}
            <div
                ref={scrollRef}
                onScroll={handleScroll}
                className="w-full flex-1 overflow-y-auto overflow-x-hidden bg-iota-neutral-100 p-md dark:bg-iota-neutral-6"
            >
                {children}
            </div>
        </div>
    );
}
