import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';

const gqlClient = new IotaGraphQLClient({
    url: 'https://graphql.testnet.iota.cafe/',
});

const queryTransactionBalanceChanges = graphql(`
    query ($address: IotaAddress!) {
        transactionBlocks(filter: {
            function: "0x3::iota_system::request_add_stake"
            signAddress: $address
        }) {
            nodes {
                digest
                effects {
                    balanceChanges {
                        nodes {
                            owner {
                                address
                            }
                            amount
                        }
                    }
                }
            }
        }
    }
`);

async function getTransactionBalanceChanges(address: string) {
    const result = await gqlClient.query({
        query: queryTransactionBalanceChanges,
        variables: {
            address,
        },
    });

    return result.data?.transactionBlocks;
}
