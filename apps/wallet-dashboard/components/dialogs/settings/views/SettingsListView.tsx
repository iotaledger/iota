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
import { Globe } from '@iota/apps-ui-icons';
import { usePersistedNetwork } from '@/hooks';
import { toTitleCase } from '@iota/core';
import { LEGAL_LINKS } from '@/lib/constants/routes.constants';
import { ExternalLink } from '@/components/ExternalLink';

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
                                    <div className="flex h-10 w-10 items-center justify-center rounded-full  text-iota-neutral-10 dark:text-iota-neutral-92 [&_svg]:h-5 [&_svg]:w-5">
                                        <span className="text-2xl">{item.icon}</span>
                                    </div>
                                </CardImage>
                                <CardBody title={item.title} subtitle={item.subtitle} />
                                <CardAction type={CardActionType.Link} />
                            </Card>
                        ))}
                    </div>
                    <div className="flex flex-col items-center gap-y-1 text-center">
                        <p>{process.env.NEXT_PUBLIC_DASHBOARD_REV}</p>
                        <div className="flex gap-md">
                            {LEGAL_LINKS.map(({ title, href }) => (
                                <ExternalLink
                                    key={href}
                                    href={href}
                                    type="legal"
                                    className="text-label-sm text-iota-primary-30 dark:text-iota-primary-80"
                                >
                                    {title}
                                </ExternalLink>
                            ))}
                        </div>
                    </div>
                </div>
            </DialogLayoutBody>
        </DialogLayout>
    );
}
