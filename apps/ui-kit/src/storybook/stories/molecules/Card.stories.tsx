// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import {
    Card,
    CardAction,
    CardImage,
    CardText,
    CardActionVariant,
    CardVariant,
    ImageType,
    ImageVariant,
} from '@/components/molecules/card';

const meta = {
    component: Card,
    subcomponents: { CardImage, CardText, CardAction },
    tags: ['autodocs'],
    render: (props) => {
        return (
            <div className={'flex gap-3'}>
                <div>
                    <Card
                        cardVariant={props.cardVariant}
                        disabled={props.disabled}
                        text={{
                            title: props?.text?.title,
                            subtitle: props?.text?.subtitle,
                        }}
                        image={{
                            type: ImageType.Placeholder,
                            variant: ImageVariant.Rounded,
                        }}
                        action={{
                            title: 'Action',
                            variant: CardActionVariant.Link,
                            onClick: () => alert('Action Clicked'),
                        }}
                    />
                </div>

                <div>
                    <Card
                        onClick={() => alert('Card Clicked')}
                        cardVariant={props.cardVariant}
                        disabled={props.disabled}
                    >
                        <CardImage {...props.image} />
                        <CardText {...props.text} />
                        <CardAction {...props.action} />
                    </Card>
                </div>
            </div>
        );
    },
} satisfies Meta<typeof Card>;

export default meta;

type Story = StoryObj<typeof meta>;

// export const Default: Story = {
//     args: {
//         cardVariant: CardVariant.Default,
//         text: {
//             title: 'Title',
//             subtitle: 'Subtitle',
//         },
//     },
//     argTypes: {
//         cardVariant: {
//             control: {
//                 type: 'select',
//                 options: [CardVariant.Default, CardVariant.Outlined, CardVariant.Filled],
//             },
//         },
//         // text: {
//         //
//         // }
//         text: {
//             title: {
//                 control: 'text',
//             },
//             subtitle: {
//                 control: 'text',
//             },
//         },
//
//         image: {
//             type: {
//                 control: 'select',
//                 options: [ImageType.Placeholder, ImageType.Img, ImageType.Icon, ImageType.IconOnly],
//             },
//         },
//     },
// };

export const Default: Story = {
    args: {
        cardVariant: CardVariant.Default,
        text: {
            title: 'Title',
            subtitle: 'Subtitle',
        },
        image: {
            type: ImageType.Placeholder,
            variant: ImageVariant.Rounded,
        },
        action: {
            title: 'Click Me!',
            variant: CardActionVariant.Link,
        },
    },
    argTypes: {
        cardVariant: {
            control: {
                type: 'select',
                options: [CardVariant.Default, CardVariant.Outlined, CardVariant.Filled],
            },
        },
        text: {
            title: {
                control: 'text',
            },
            subtitle: {
                control: 'text',
            },
        },
        image: {
            type: {
                control: 'select',
                options: [ImageType.Placeholder, ImageType.Img, ImageType.Icon, ImageType.IconOnly],
            },
            variant: {
                control: 'select',
                options: Object.values(ImageVariant), // Assuming ImageVariant is an enum or object
            },
        },
        action: {
            title: {
                control: 'text',
            },
            variant: {
                control: 'select',
                options: Object.values(CardActionVariant), // Assuming CardActionVariant is an enum or object
            },
            onClick: {
                action: 'clicked',
            },
        },
    },
};
