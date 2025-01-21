// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { IotaLogoSmall } from '@iota/apps-ui-icons';
import {
    Card,
    CardProps,
    CardImage,
    CardImageProps,
    CardAction,
    CardBody,
    CardBodyProps,
    CardActionProps,
    CardActionType,
    CardType,
    ImageType,
    ImageShape,
} from '@/components';

type CardCustomProps = CardProps & {
    imageType: CardImageProps['type'];
    imageUrl: CardImageProps['url'];
    imageVariant: CardImageProps['shape'];
    textTitle: CardBodyProps['title'];
    textSubtitle: CardBodyProps['subtitle'];
    actionTitle: CardActionProps['title'];
    actionSubtitle: CardActionProps['subtitle'];
    actionType: CardActionProps['type'];
};

const meta: Meta<CardCustomProps> = {
    component: Card,
    tags: ['autodocs'],
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
        options: Object.values(ImageShape),
    },
    actionType: {
        control: 'select',
        options: Object.values(CardActionType),
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
    actionType: CardActionType.Link,
    variant: CardType.Default,
    isDisabled: false,
    imageVariant: ImageShape.Rounded,
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
            <Card isDisabled={args.isDisabled} type={args.type} onClick={args.onClick}>
                <CardImage type={args.imageType} shape={args.imageVariant} url={args.imageUrl} />
                <CardBody title={args.textTitle} subtitle={args.textSubtitle} />
                <CardAction
                    title={args.actionTitle}
                    subtitle={args.actionSubtitle}
                    type={args.actionType}
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
            <Card isDisabled={args.isDisabled} type={args.type} onClick={args.onClick}>
                <CardImage type={args.imageType} shape={args.imageVariant} url={args.imageUrl}>
                    <IotaLogoSmall />
                </CardImage>
                <CardBody title={args.textTitle} subtitle={args.textSubtitle} />
                <CardAction
                    title={args.actionTitle}
                    subtitle={args.actionSubtitle}
                    type={args.actionType}
                    onClick={args.onClick}
                />
            </Card>
        );
    },
};
