import { useQuery } from '@apollo/client';
import { graphql } from '@iota/iota-sdk/graphql/schemas/2025.2';

const chainIdentifierQuery = graphql(`
    query {
        chainIdentifier
    }
`);

function ChainIdentifier() {
    const { loading, error, data } = useQuery(chainIdentifierQuery);

    return <div>{data?.chainIdentifier}</div>;
}
