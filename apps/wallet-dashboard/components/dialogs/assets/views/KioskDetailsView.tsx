// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useGetKioskContents,
    getKioskIdFromOwnerCap,
    useNftDetails,
    NftImage,
    Collapsible,
    ExplorerLinkType,
} from '@iota/core';
import { Header, KeyValueInfo, LoadingIndicator } from '@iota/apps-ui-kit';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useCurrentAccount } from '@iota/dapp-kit';
import { ExplorerLink } from '@/components/ExplorerLink';
import { formatAddress } from '@iota/iota-sdk/utils';

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
                <div className="mb-auto grid grid-cols-3 items-center justify-center gap-3">
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
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <Collapsible defaultOpen title="Details">
                    <div className="flex flex-col gap-y-sm px-md py-xs">
                        <KeyValueInfo
                            keyText="Number of Items"
                            value={items?.length || '0'}
                            fullwidth
                        />
                        <KeyValueInfo
                            keyText="Kiosk ID"
                            value={
                                <ExplorerLink objectID={objectId!} type={ExplorerLinkType.Object}>
                                    {formatAddress(objectId!)}
                                </ExplorerLink>
                            }
                            fullwidth
                        />
                    </div>
                </Collapsible>
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
