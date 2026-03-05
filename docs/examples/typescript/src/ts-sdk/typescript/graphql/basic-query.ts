import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';

const gqlClient = new IotaGraphQLClient({
    url: 'https://graphql.testnet.iota.cafe/',
});

const chainIdentifierQuery = graphql(`
    query {
        chainIdentifier
    }
`);

async function getChainIdentifier() {
    const result = await gqlClient.query({
        query: chainIdentifierQuery,
        variables: {},
    });

    return result.data?.chainIdentifier;
}
getChainIdentifier()
    .then(identifier => console.log('Chain Identifier:', identifier))
    .catch(error => console.error('Error:', error));
