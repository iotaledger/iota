import { IotaClient } from '@iota/iota-sdk/client';
declare const client: IotaClient;

const page1 = await client.getCheckpoints({
    limit: 10,
    descendingOrder: false,
});

const page2 =
    page1.hasNextPage &&
    client.getCheckpoints({
        cursor: page1.nextCursor,
        limit: 10,
        descendingOrder: false,
    });
