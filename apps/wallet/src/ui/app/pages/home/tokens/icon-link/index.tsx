// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cl from 'clsx';
import { memo } from 'react';
import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';
import st from './IconLink.module.scss';

export interface IconLinkProps {
    to: string;
    icon: ReactNode;
    disabled?: boolean;
    text: string;
}

function IconLinkComponent({ to, icon, disabled = false, text }: IconLinkProps) {
    return (
        <Link
            to={to}
            className={cl(st.container, { [st.disabled]: disabled })}
            tabIndex={disabled ? -1 : undefined}
        >
            <div className={cl(disabled ? 'text-gray-60' : 'text-hero-dark')}>{icon}</div>
            <span
                className={cl(
                    'text-body-sm',
                    disabled ? 'opacity-60' : 'text-neutral-10 dark:text-neutral-92',
                )}
            >
                {text}
            </span>
        </Link>
    );
}

export const IconLink = memo(IconLinkComponent);
