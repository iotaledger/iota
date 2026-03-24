import { getGraphQLUrl, Network } from '@iota/iota-sdk/client';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';

import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';

async function main() {
    try {

        const network = Network.Testnet;
        const graphqlUrl = getGraphQLUrl(network);
        if (!graphqlUrl) {
            throw new Error(
                `GraphQL endpoint not configured for ${network}.`
            );
        }

        console.log(`Connecting to GraphQL endpoint: ${graphqlUrl}`);

        const gqlClient = new IotaGraphQLClient({
            url: graphqlUrl,
        });

        const result = await gqlClient.query({
            query: graphql(`
                query {
                    chainIdentifier
                }
            `),
            variables: {},
        });

        if (!result.data?.chainIdentifier) {
            throw new Error('No chain identifier returned from query');
        }

        console.log('Chain Identifier:', result.data.chainIdentifier);
    } catch (error) {
        console.error('Error:', error instanceof Error ? error.message : error);
        process.exit(1);
    }
}

main().catch(e => {
    console.error('Unhandled promise rejection:', e);
    process.exit(1);
});
