// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'clsx';

import { nftDisplayVariants } from '@/components/name-record/variants';
import { MenuListItem, type NftDisplayProps } from '@/lib/types/components';

import { ContextMenuButton } from '../buttons/ContextMenuButton';
import { AvatarDisplay, NameAvatarDisplay } from '../name-record/AvatarDisplay';

interface NameCardProps extends NftDisplayProps {
    menuOptions?: MenuListItem[];
    isSelected?: boolean;
    displaySrc?: string | null;
    blurImage?: boolean;
    testId?: string;
}

export function NameCard({
    name,
    badge,
    size,
    menuOptions,
    isSelected,
    children,
    displaySrc,
    blurImage,
    testId,
}: React.PropsWithChildren<NameCardProps>) {
    return (
        <div
            className={cx(
                'relative group/name-card rounded-xl name-card-shadow bg-names-neutral-6 overflow-visible',
                isSelected && 'name-card-selected',
                nftDisplayVariants({ size }),
            )}
            data-testid={testId}
        >
            <div
                className={cx(
                    'flex flex-col relative aspect-square rounded-xl group/display z-0 overflow-hidden w-full',
                )}
                data-testid="name-card-avatar"
            >
                {displaySrc ? (
                    <AvatarDisplay src={displaySrc} blur={blurImage} />
                ) : (
                    <NameAvatarDisplay name={name} blur={blurImage} />
                )}

                {badge && <div className="absolute top-sm left-sm">{badge}</div>}
                {menuOptions?.length ? (
                    <div className="opacity-0 group-hover/display:opacity-100 transition-opacity absolute w-full top-0 right-0 px-sm py-sm bg-gradient-to-b from-black/80 to-transparent flex justify-end pointer-events-none">
                        <div className="shadow-xl pointer-events-auto">
                            <ContextMenuButton options={menuOptions} />
                        </div>
                    </div>
                ) : null}
            </div>

            <div className={nftDisplayVariants({ size })}>{children}</div>
        </div>
    );
}
