// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaDocument } from '@iota/identity-wasm/web';
import { validateDomainLinkageByEndpoint } from '../identityServiceHelper';
import { useQuery } from '@tanstack/react-query';

export const getValidateDomainLinkageQuery = (
    issuerDocument: IotaDocument,
    endpointUrl: string,
) => ({
    queryKey: ['did-jwt', issuerDocument, endpointUrl],
    async queryFn() {
        return validateDomainLinkageByEndpoint(issuerDocument, endpointUrl);
    },
    enabled: !!issuerDocument && !!endpointUrl,
});

export function useValidateDomainLinkage(issuerDocument: IotaDocument, endpointUrl: string) {
    return useQuery<boolean>(getValidateDomainLinkageQuery(issuerDocument, endpointUrl));
}
