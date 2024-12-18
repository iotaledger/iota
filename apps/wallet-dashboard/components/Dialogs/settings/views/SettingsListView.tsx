// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
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
import { getNetwork } from '@iota/iota-sdk/client';
import { useIotaClientContext } from '@iota/dapp-kit';
import { Globe } from '@iota/ui-icons';

interface SettingsListViewProps {
    handleClose: () => void;
    setView: (view: SettingsDialogView) => void;
}

export function SettingsListView({ handleClose, setView }: SettingsListViewProps): JSX.Element {
    const { network } = useIotaClientContext();
    const { name: networkName } = getNetwork(network);
    function onSelectSettingClick(view: SettingsDialogView): void {
        setView(view);
    }
    const MENU_ITEMS = [
        {
            title: 'Network',
            subtitle: networkName,
            icon: <Globe />,
            onClick: () => onSelectSettingClick(SettingsDialogView.NetworkSettings),
        },
    ];
    return (
        <DialogLayout>
            <Header title="Settings" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
            <div className="flex flex-col content-stretch h-full">
                <div className="flex w-full flex-col gap-md h-full">
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
