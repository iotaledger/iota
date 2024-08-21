// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
} from '@iota/apps-ui-kit';

export interface ItemProps {
    icon: React.ReactNode;
    title: string;
    subtitle?: string;
    onClick?: () => void;
    isDisabled?: boolean;
    isHidden?: boolean;
}

function MenuListItem({ icon, title, subtitle, onClick, isDisabled, isHidden }: ItemProps) {
    return (
        !isHidden && (
            <Card type={CardType.Default} onClick={onClick} isDisabled={isDisabled}>
                <CardImage type={ImageType.BgSolid}>
                    <div className="flex h-10 w-10 items-center justify-center rounded-full  text-neutral-10 [&_svg]:h-5 [&_svg]:w-5">
                        <span className="text-2xl">{icon}</span>
                    </div>
                </CardImage>
                <CardBody title={title} subtitle={subtitle} />
                <CardAction type={CardActionType.Link} />
            </Card>
        )
    );
}

export default MenuListItem;
