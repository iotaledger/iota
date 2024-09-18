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
    ImageShape,
    ImageType,
} from '@iota/apps-ui-kit';

interface WalletListItemProps {
    name: string;
    icon: React.ReactNode;
    isSelected?: boolean;
    onClick: () => void;
}

export function WalletListItem({ name, icon, isSelected, onClick }: WalletListItemProps) {
    return (
        <Card type={!isSelected ? CardType.Default : CardType.Filled} onClick={onClick}>
            <CardImage type={ImageType.Placeholder} shape={ImageShape.SquareRounded}>
                {typeof icon === 'string' ? <img src={icon} alt={`${name} logo`} /> : icon}
            </CardImage>
            <CardBody title={name} />
            <CardAction type={CardActionType.Link} />
        </Card>
    );
}
