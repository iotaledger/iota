// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useGetKioskContents,
    getKioskIdFromOwnerCap,
    useNftDetails,
    NftImage,
    ExplorerLinkType,
    ViewTxnOnExplorerButton,
    OutlinedCopyButton,
    toast,
} from '@iota/core';
import { Badge, BadgeType, Header, LoadingIndicator } from '@iota/apps-ui-kit';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useCurrentAccount } from '@iota/dapp-kit';
import { ExplorerLink } from '@/components/ExplorerLink';

interface DetailsViewProps {
    asset: IotaObjectData;
    onClose: () => void;
    onItemClick: (asset: IotaObjectData) => void;
}

export function KioskDetailsView({ onClose, asset, onItemClick }: DetailsViewProps) {
    const account = useCurrentAccount();
    const senderAddress = account?.address ?? '';
    const objectId = getKioskIdFromOwnerCap(asset);
    const { data: kioskData, isPending } = useGetKioskContents(account?.address);
    const kiosk = kioskData?.kiosks.get(objectId);
    const items = kiosk?.items;

    if (isPending) {
        return (
            <div className="flex h-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <>
            <Header title="Kiosk" onClose={onClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex flex-col gap-md">
                    <div className="flex flex-row gap-x-sm">
                        <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                            Kiosk items
                        </span>
                        <Badge type={BadgeType.Neutral} label={items?.length.toString() ?? '0'} />
                    </div>
                    <div className="grid grid-cols-3 items-center justify-center gap-sm">
                        {items?.map((item) => {
                            return item.data?.objectId ? (
                                <div
                                    onClick={() => {
                                        item.data && onItemClick(item.data);
                                    }}
                                    key={item.data?.objectId}
                                >
                                    <KioskItem object={item.data} address={senderAddress} />
                                </div>
                            ) : null;
                        })}
                    </div>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <div className="flex w-full flex-row gap-x-xs">
                    <div className="flex w-full [&_a]:w-full">
                        <ExplorerLink objectID={objectId} type={ExplorerLinkType.Object}>
                            <ViewTxnOnExplorerButton digest={objectId} />
                        </ExplorerLink>
                    </div>
                    <div className="self-center">
                        <OutlinedCopyButton
                            textToCopy={objectId ?? ''}
                            onCopySuccess={() =>
                                toast.success('Kiosk Object ID copied to clipboard')
                            }
                        />
                    </div>
                </div>
            </DialogLayoutFooter>
        </>
    );
}

interface KioskItemProps {
    object: IotaObjectData;
    address: string;
}

function KioskItem({ object, address }: KioskItemProps) {
    const { nftName, nftImageUrl } = useNftDetails(object.objectId, address);

    return <NftImage title={nftName} src={nftImageUrl} isHoverable />;
}
