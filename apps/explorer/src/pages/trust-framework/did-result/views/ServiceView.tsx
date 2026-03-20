// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Info } from '@iota/apps-ui-icons';
import { InfoBox, InfoBoxStyle, InfoBoxType, Title } from '@iota/apps-ui-kit';
import { type IotaDocument } from '@iota/identity-wasm/web';
import { Link, ListItem } from '~/components';

interface ServiceViewProps {
    didDocument: IotaDocument;
}

export function ServiceView({ didDocument }: ServiceViewProps) {
    const infoDomainLinkage = didDocument
        .service()
        .filter((service) => service.type().includes('LinkedDomains'))
        .map((service) => ({
            id: service.id().toString(),
            endpoint: service.serviceEndpoint(),
        }));

    return (
        <div className="flex w-full flex-col gap-sm">
            <Title title="Domain Linkage" />
            <div className="flex flex-col">
                {!infoDomainLinkage.length && (
                    <InfoBox
                        supportingText="No linked domain registered."
                        icon={<Info />}
                        type={InfoBoxType.Default}
                        style={InfoBoxStyle.Elevated}
                    />
                )}
                {infoDomainLinkage.map((dlItem) => (
                    <ListItem key={dlItem.id}>
                        <div className="flex w-full flex-row">
                            {/* NOTE: For now an endpoint is only a string, but it can change */}
                            <Link to={dlItem.endpoint as string}>{dlItem.endpoint}</Link>
                        </div>
                    </ListItem>
                ))}
            </div>
        </div>
    );
}
