// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CloseFilled, CheckmarkFilled, Info } from '@iota/apps-ui-icons';
import {
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Title,
    Chip,
    ChipSize,
    ChipType,
    LoadingIndicator,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { type IotaDocument } from '@iota/identity-wasm/web';
import { useValidateDomainLinkage } from '../hooks/useValidateDomainLinkage';
import { getDidConfigurationUrl } from '../identityServiceHelper';

interface ServiceViewProps {
    didDocument: IotaDocument;
}

/**
 * Renders a view for the services of a DID document, specifically filtering for and displaying "LinkedDomains" services.
 * It shows a title, a list of linked domain endpoints, or an informational message if no linked domains are found.
 *
 * @param {ServiceViewProps} props The component props.
 * @param {IotaDocument} props.didDocument The DID document to extract the service information from.
 * @returns {JSX.Element} The rendered component.
 */
export function ServiceView({ didDocument }: ServiceViewProps) {
    const infoDomainLinkage = didDocument
        .service()
        .filter((service) => service.type().includes('LinkedDomains'))
        .map((service) => ({
            id: service.id().toString(),
            endpoint: service.serviceEndpoint() as string,
        }));

    return (
        <div className="flex w-full flex-col gap-sm">
            <Title
                title="Domain Linkage"
                tooltipPosition={TooltipPosition.Left}
                tooltipText="A verified, bidirectional connection between this Identity and a web domain. Proves that the Identity controller owns the linked domain."
            />
            <div className="flex flex-wrap gap-2">
                {!infoDomainLinkage.length && (
                    <InfoBox
                        supportingText="No linked domain registered."
                        icon={<Info />}
                        type={InfoBoxType.Default}
                        style={InfoBoxStyle.Elevated}
                    />
                )}
                {infoDomainLinkage.map(({ id, endpoint }) => (
                    <DomainLinkage key={id} endpoint={endpoint} didDocument={didDocument} />
                ))}
            </div>
        </div>
    );
}

interface DomainLinkageProps {
    endpoint: string;
    didDocument: IotaDocument;
}
function DomainLinkage({ endpoint, didDocument }: DomainLinkageProps) {
    const { data: isValid, isPending, isSuccess } = useValidateDomainLinkage(didDocument, endpoint);

    return (
        <>
            {isPending && <LoadingDomainLinkage endpoint={endpoint} />}
            {isSuccess && <ValidatedDomainLinkage endpoint={endpoint} isValid={isValid} />}
        </>
    );
}

interface LoadingDomainLinkageProps {
    endpoint: string;
}

function LoadingDomainLinkage({ endpoint }: LoadingDomainLinkageProps) {
    return (
        <Chip
            type={ChipType.Outline}
            label={endpoint as string}
            size={ChipSize.Small}
            trailingElement={<ValidatingDomainLinkageIcon />}
            disabled
        />
    );
}

interface ValidatedDomainLinkageProps {
    endpoint: string;
    isValid: boolean;
}

function ValidatedDomainLinkage({ endpoint, isValid }: ValidatedDomainLinkageProps) {
    const didConfigurationUrl = getDidConfigurationUrl(endpoint).toString();

    return (
        <a href={didConfigurationUrl} target="_blank" rel="noopener noreferrer">
            <Chip
                type={ChipType.Outline}
                label={endpoint as string}
                size={ChipSize.Small}
                trailingElement={
                    isValid ? <ValidDomainLinkageIcon /> : <InvalidDomainLinkageIcon />
                }
            />
        </a>
    );
}

function ValidatingDomainLinkageIcon() {
    return <LoadingIndicator />;
}

function ValidDomainLinkageIcon() {
    return <CheckmarkFilled className="text-on-success" />;
}

function InvalidDomainLinkageIcon() {
    return <CloseFilled className="text-on-error" />;
}
