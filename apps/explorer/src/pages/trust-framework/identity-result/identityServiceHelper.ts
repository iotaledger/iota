// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    DomainLinkageConfiguration,
    EcDSAJwsVerifier,
    type IotaDocument,
    Jwt,
    JwtCredentialValidationOptions,
    JwtDomainLinkageValidator,
} from '@iota/identity-wasm/web';
import { type FetchResult, type DomainLinkageResource } from './types';
import { z } from 'zod';

// An extensible list of supported context versions for the DID Configuration Resource.
const SUPPORTED_DID_CONFIGURATION_CONTEXTS = [
    'https://identity.foundation/.well-known/did-configuration/v1',
] as const;

// Define tolerance limits for the fetch request.
const FETCH_TOLERANCES = {
    TIMEOUT_MS: 5000, // 5 seconds
    MAX_SIZE_BYTES: 1024 * 10, // 10 KB
};

export function getDidConfigurationUrl(endpoint: string): URL {
    return new URL('/.well-known/did-configuration.json', endpoint);
}

export async function fetchDidConfigurationJson(
    endpoint: string,
): Promise<FetchResult<DomainLinkageResource>> {
    const configurationUrl = getDidConfigurationUrl(endpoint);

    // Use AbortController for request timeout.
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), FETCH_TOLERANCES.TIMEOUT_MS);

    let response: Response;
    try {
        response = await fetch(configurationUrl, {
            method: 'GET',
            headers: {
                Accept: 'application/json',
            },
            signal: controller.signal,
        });
    } catch (error) {
        if (error instanceof Error && error.name === 'AbortError') {
            console.warn(`Request timed out after ${FETCH_TOLERANCES.TIMEOUT_MS}ms`);
            return {
                isSuccess: false,
                isError: true,
                errorMsg: 'Request timed out.',
            };
        }
        console.warn('Network error during fetch:', error);
        return {
            isSuccess: false,
            isError: true,
            errorMsg: 'A network error occurred.',
        };
    } finally {
        clearTimeout(timeoutId);
    }

    if (!response.ok) {
        console.warn('Failed to get configuration URL', response.statusText);
        return {
            isSuccess: false,
            isError: true,
            errorMsg: 'Failed to fetch DID configuration.',
        };
    }

    // 1. Validate Content-Length to prevent processing excessively large files.
    const contentLength = response.headers.get('Content-Length');
    if (contentLength && parseInt(contentLength, 10) > FETCH_TOLERANCES.MAX_SIZE_BYTES) {
        console.warn(
            `Response size (${contentLength} bytes) exceeds the limit of ${FETCH_TOLERANCES.MAX_SIZE_BYTES} bytes.`,
        );
        return {
            isSuccess: false,
            isError: true,
            errorMsg: 'Response size exceeds the limit.',
        };
    }

    // 2. Validate Content-Type to ensure we are processing JSON.
    const contentType = response.headers.get('Content-Type');
    if (!contentType || !contentType.startsWith('application/json')) {
        console.warn(`Unexpected Content-Type: ${contentType}`);
        return {
            isSuccess: false,
            isError: true,
            errorMsg: 'Invalid Content-Type.',
        };
    }

    const data = await response.json();

    // 3. Validate the structure and values of the received JSON.
    const DomainLinkageResourceSchema = z
        .object({
            '@context': z.enum(SUPPORTED_DID_CONFIGURATION_CONTEXTS),
            linked_dids: z.array(z.string()).nonempty(),
        })
        .strict();

    const parsed = DomainLinkageResourceSchema.safeParse(data);

    if (!parsed.success) {
        console.warn('Invalid or unsupported DID configuration JSON structure', parsed.error);
        return {
            isSuccess: false,
            isError: true,
            errorMsg: 'Invalid or unsupported DID configuration JSON structure.',
        };
    }

    return {
        isSuccess: true,
        isError: false,
        data: parsed.data,
    };
}

export function getFirstDomainLinkageConfiguration(linkedDids: string[]) {
    const firstDidJwt = 0;
    const didJwt = linkedDids.at(firstDidJwt);
    const domainLinkageConfiguration = new DomainLinkageConfiguration([Jwt.fromJSON(didJwt)]);
    return domainLinkageConfiguration;
}

export async function validateDomainLinkageByEndpoint(
    issuerDocument: IotaDocument,
    endpointUrl: string,
) {
    const { data: didConfigurationJson, isError } = await fetchDidConfigurationJson(endpointUrl);
    if (isError || !didConfigurationJson) {
        return false;
    }

    try {
        const domainLinkageConfiguration = getFirstDomainLinkageConfiguration(
            didConfigurationJson.linked_dids,
        );

        // Throws if invalid
        new JwtDomainLinkageValidator(new EcDSAJwsVerifier()).validateLinkage(
            issuerDocument,
            domainLinkageConfiguration,
            endpointUrl,
            new JwtCredentialValidationOptions(),
        );

        return true;
    } catch {
        console.error('Invalid Domain Linkage');
        return false;
    }
}
