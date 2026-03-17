import { useIotaClientContext } from '@iota/dapp-kit';

function NetworkSelector() {
    const ctx = useIotaClientContext();

    return (
        <div>
            {Object.keys(ctx.networks).map((network) => (
                <button key={network} onClick={() => ctx.selectNetwork(network)}>
                    {`select ${network}`}
                </button>
            ))}
        </div>
    );
}

export default NetworkSelector;
