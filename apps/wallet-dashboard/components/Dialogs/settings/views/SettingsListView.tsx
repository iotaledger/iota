// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    Header,
    ImageType,
} from '@iota/apps-ui-kit';
import { DialogLayout, DialogLayoutBody } from '../../layout';
import { SettingsDialogView } from '../enums';
import { Globe } from '@iota/ui-icons';
import { usePersistedNetwork } from '@/hooks';
import { toTitleCase } from '@iota/core';

interface SettingsListViewProps {
    handleClose: () => void;
    setView: (view: SettingsDialogView) => void;
}

export function SettingsListView({ handleClose, setView }: SettingsListViewProps): JSX.Element {
    const { persistedNetwork } = usePersistedNetwork();
    const MENU_ITEMS = [
        {
            title: 'Network',
            subtitle: toTitleCase(persistedNetwork),
            icon: <Globe />,
            onClick: () => setView(SettingsDialogView.NetworkSettings),
        },
    ];

    return (
        <DialogLayout>
            <Header title="Settings" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex h-full flex-col content-stretch">
                    <div className="flex h-full w-full flex-col gap-md">
                        {MENU_ITEMS.map((item, index) => (
                            <Card key={index} type={CardType.Default} onClick={item.onClick}>
                                <CardImage type={ImageType.BgSolid}>
                                    <div className="flex h-10 w-10 items-center justify-center rounded-full  text-neutral-10 dark:text-neutral-92 [&_svg]:h-5 [&_svg]:w-5">
                                        <span className="text-2xl">{item.icon}</span>
                                    </div>
                                </CardImage>
                                <CardBody title={item.title} subtitle={item.subtitle} />
                                <CardAction type={CardActionType.Link} />
                            </Card>
                        ))}
                    </div>
                    <p className="text-center">{process.env.NEXT_PUBLIC_DASHBOARD_REV}</p>
                </div>
            </DialogLayoutBody>
        </DialogLayout>
    );
}
