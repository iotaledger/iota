import { bcs } from '@iota/iota-sdk/bcs';

export const Receipt = bcs.struct('Receipt', {
    RequestID: bcs.Address,
});

export const AssetsBag = bcs.struct('AssetsBag', {
    ID: bcs.Address,
    Size: bcs.u64(),
});

export const ReferentAssetsBag = bcs.struct('ReferentAssetsBag', {
    ID: bcs.Address,
    Value: bcs.option(AssetsBag),
});

export const Anchor = bcs.struct('Anchor', {
    UID: bcs.Address,
    Assets: ReferentAssetsBag,
    StateMetadata: bcs.vector(bcs.u8()),
    StateIndex: bcs.u32(),
});

export const Message = bcs.struct('Message', {
    Contract: bcs.u32(),
    Function: bcs.u32(),
    Args: bcs.vector(bcs.vector(bcs.u8())),
});

export const CoinAllowance = bcs.struct('CoinAllowance', {
    CoinType: bcs.string(),
    Balance: bcs.u64(),
});

export const Request = bcs.struct('Request', {
    UID: bcs.Address,
    Sender: bcs.Address,
    AssetsBag: ReferentAssetsBag,
    Message: Message,
    Allowance: bcs.vector(CoinAllowance),
    GasBudget: bcs.u64(),
});

export const RequestEvent = bcs.struct('RequestEvent', {
    RequestID: bcs.Address,
    Anchor: bcs.Address,
});
