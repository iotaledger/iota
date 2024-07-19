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
    // imageIconName: CardImageProps['iconName'];
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

export const Default: Story = {
    args: {
        imageType: ImageType.Placeholder,
        imageUrl: 'https://via.placeholder.com/150.png',
        imageVariant: ImageVariant.Rounded,
        // imageIconName: '',
        disabled: false,
        variant: CardVariant.Default,
        textTitle: 'Card Title',
        textSubtitle: 'Card Subtitle',
        actionTitle: 'Action title',
        actionSubtitle: 'Action subtitle',
        actionVariant: CardActionVariant.Link,
    },
    argTypes: {
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
    },
    render: (args) => {
        return (
            <Card disabled={args.disabled} variant={args.variant} onClick={args.onClick}>
                <CardImage
                    type={args.imageType}
                    variant={args.imageVariant}
                    url={args.imageUrl}
                    // iconName={args.imageIconName}
                />
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
        imageType: ImageType.BgSolid,
        imageVariant: ImageVariant.Rounded,
        disabled: false,
        variant: CardVariant.Default,
        textTitle: 'Card Title',
        textSubtitle: 'Card Subtitle',
        actionTitle: 'Action title',
        actionSubtitle: 'Action subtitle',
        actionVariant: CardActionVariant.Link,
    },
    argTypes: {
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
    },
    render: (args) => {
        return (
            <Card disabled={args.disabled} variant={args.variant} onClick={args.onClick}>
                <CardImage
                    type={args.imageType}
                    variant={args.imageVariant}
                    url={args.imageUrl}
                    icon={<IotaLogoSmall />}
                />
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
