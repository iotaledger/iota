// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ButtonUnstyled,
    Card,
    CardImage,
    CardType,
    Divider,
    ImageShape,
    ImageType,
} from '@iota/apps-ui-kit';
import { normalizeIotaName } from '@iota/iota-names-sdk';
import clsx from 'clsx';

import { NameAvatarDisplay } from '@/components/name-record/AvatarDisplay';

import { PanelTileType } from './enums';

const CARD_BG_COLOR: Record<PanelTileType, string> = {
    [PanelTileType.Default]: 'bg-transparent',
    [PanelTileType.Warning]: 'bg-[#342109]',
    [PanelTileType.Destructive]: 'bg-[#47000C]',
};

interface PanelTileProps {
    name: string;
    type?: PanelTileType;
    subtitle?: string;
    footer?: React.ReactNode;
    icon?: React.ReactNode;
    onClick?: React.ComponentProps<typeof Card>['onClick'];
    menuButton?: React.ReactNode;
}

export function PanelTile({
    name,
    type = PanelTileType.Default,
    subtitle,
    footer,
    icon,
    onClick,
    menuButton,
}: PanelTileProps) {
    const title = normalizeIotaName(name, 'at', { truncateLongParts: true });

    return (
        <div className={clsx('flex flex-col rounded-xl overflow-hidden', CARD_BG_COLOR[type])}>
            <div className="relative">
                <Card type={CardType.Default}>
                    <div className="flex flex-row items-center w-full gap-sm max-w-full">
                        <ButtonUnstyled
                            className="state-layer flex flex-row w-full items-center max-w-full min-w-0 gap-sm"
                            onClick={onClick}
                        >
                            <CardImage
                                type={ImageType.BgTransparent}
                                shape={ImageShape.SquareRounded}
                            >
                                <NameAvatarDisplay name={name} />
                            </CardImage>

                            <div className="flex flex-col gap-xxs w-full min-w-0">
                                <div className="flex flex-row items-center max-w-full">
                                    <div className="text-title-md font-medium leading-[120%] tracking-[-0.15px] text-names-neutral-92 break-words text-left min-w-0">
                                        {title}
                                    </div>

                                    {icon && <span className="ml-2 flex">{icon}</span>}
                                </div>

                                {subtitle && (
                                    <div
                                        data-amp-mask
                                        className="text-left text-title-sm font-normal text-names-neutral-60 break-words"
                                    >
                                        {subtitle}
                                    </div>
                                )}
                            </div>
                        </ButtonUnstyled>

                        {menuButton}
                    </div>
                </Card>
            </div>

            {footer && <Divider />}
            {footer}
        </div>
    );
}
