// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { IotaLogoSmall } from '@iota/ui-icons';
import {
    Card,
    CardProps,
    CardImage,
    CardImageProps,
    CardAction,
    CardText,
    CardTextProps,
    CardActionProps,
} from '@/components/molecules/card';
import {
    CardActionVariant,
    CardVariant,
    ImageType,
    ImageVariant,
} from '@/components/molecules/card/card.enums';

type CardCustomProps = CardProps & {
    test: string;
    imageType: CardImageProps['type'];
    imageUrl: CardImageProps['url'];
    imageVariant: CardImageProps['variant'];
    textTitle: CardTextProps['title'];
    textSubtitle: CardTextProps['subtitle'];
    actionTitle: CardActionProps['title'];
    actionSubtitle: CardActionProps['subtitle'];
    actionVariant: CardActionProps['variant'];
};

const meta: Meta<CardCustomProps> = {
    component: Card,
};

export default meta;

type Story = StoryObj<typeof meta>;

const COMMON_ARG_TYPES = {
    imageType: {
        control: 'select',
        options: Object.values(ImageType),
    },
    imageVariant: {
        control: 'select',
        options: Object.values(ImageVariant),
    },
    actionVariant: {
        control: 'select',
        options: Object.values(CardActionVariant),
    },
    onClick: {
        action: 'clicked',
    },
};

const COMMON_ARGS = {
    textTitle: 'Card Title',
    textSubtitle: 'Card Subtitle',
    actionTitle: 'Action title',
    actionSubtitle: 'Action subtitle',
    actionVariant: CardActionVariant.Link,
    variant: CardVariant.Default,
    disabled: false,
    imageVariant: ImageVariant.Rounded,
};

export const Default: Story = {
    args: {
        ...COMMON_ARGS,
        imageType: ImageType.Placeholder,
        imageUrl: 'https://via.placeholder.com/150.png',
    },
    argTypes: COMMON_ARG_TYPES,
    render: (args) => {
        return (
            <Card disabled={args.disabled} variant={args.variant} onClick={args.onClick}>
                <CardImage type={args.imageType} variant={args.imageVariant} url={args.imageUrl} />
                <CardText title={args.textTitle} subtitle={args.textSubtitle} />
                <CardAction
                    title={args.actionTitle}
                    subtitle={args.actionSubtitle}
                    variant={args.actionVariant}
                    onClick={args.onClick}
                />
            </Card>
        );
    },
};

export const WithIcon: Story = {
    args: {
        ...COMMON_ARGS,
        imageType: ImageType.BgSolid,
    },
    argTypes: COMMON_ARG_TYPES,
    render: (args) => {
        return (
            <Card disabled={args.disabled} variant={args.variant} onClick={args.onClick}>
                <CardImage type={args.imageType} variant={args.imageVariant} url={args.imageUrl}>
                    <IotaLogoSmall />
                </CardImage>
                <CardText title={args.textTitle} subtitle={args.textSubtitle} />
                <CardAction
                    title={args.actionTitle}
                    subtitle={args.actionSubtitle}
                    variant={args.actionVariant}
                    onClick={args.onClick}
                />
            </Card>
        );
    },
};
