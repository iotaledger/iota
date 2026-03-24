import { useIotaClient } from '@iota/dapp-kit';

function MyComponent() {
    const client = useIotaClient();

    // use the client
    console.log(client);
}

export default MyComponent;
