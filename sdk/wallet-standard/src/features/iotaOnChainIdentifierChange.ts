import { ChainIdentifier } from '../types';

/**
 * A Wallet Standard feature for subscribing to chain identifier changes.
 * This feature allows dApps to be notified when the wallet changes its network/chain.
 */
export type IotaOnChainIdentifierChangeFeature = {
    /** Namespace for the feature. */
    'iota:onChainIdentifierChange': {
        /** Version of the feature API. */
        version: '1.0.0';
        OnChainIdentifierChange: IotaOnChainIdentifierChangeMethod;
    };
};

/** Method to register a callback for chain identifier changes */
export type IotaOnChainIdentifierChangeMethod = (
    input: IotaOnChainIdentifierChangeInput,
) => Promise<void>;

/** Callback function that receives the new chain identifier when it changes */
export type IotaOnChainIdentifierChangeInput = (newChainIdentifier: ChainIdentifier) => void;
