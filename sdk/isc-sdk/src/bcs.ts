import { bcs } from '@iota/iota-sdk/bcs';

export const IotagoObjectRef = bcs.struct('IotagoObjectRef', {
    objectID: bcs.fixedArray(32, bcs.u8()),
    version: bcs.u64(),
    digest: bcs.vector(bcs.u8()),
});

export const IscmoveAssetsBag = bcs.struct('IscmoveAssetsBag', {
    iD: bcs.fixedArray(32, bcs.u8()),
    size: bcs.u64(),
});

export const IscAssets = bcs.struct('IscAssets', {
    coins: bcs.map(bcs.string(), bcs.u64()),
    objects: bcs.map(bcs.fixedArray(32, bcs.u8()), bcs.string()),
});

export const IscmoveReferent_AssetsBag = bcs.struct('IscmoveReferent_AssetsBag', {
    iD: bcs.fixedArray(32, bcs.u8()),
    value: IscmoveAssetsBag,
});

export const IscmoveAnchor = bcs.struct('IscmoveAnchor', {
    iD: bcs.fixedArray(32, bcs.u8()),
    assets: IscmoveReferent_AssetsBag,
    stateMetadata: bcs.vector(bcs.u8()),
    stateIndex: bcs.u32(),
});

export const IscmoveRefWithObject_Anchor = bcs.struct('IscmoveRefWithObject_Anchor', {
    objectRef: IotagoObjectRef,
    object: IscmoveAnchor,
    owner: bcs.fixedArray(32, bcs.u8()),
});

export const IscStateAnchor = bcs.struct('IscStateAnchor', {
    anchor: IscmoveRefWithObject_Anchor,
    iscPackage: bcs.fixedArray(32, bcs.u8()),
});

export const CoinType = bcs.struct('CoinType', {
    s: bcs.string(),
});

export const ParametersBaseToken = bcs.struct('ParametersBaseToken', {
    name: bcs.string(),
    tickerSymbol: bcs.string(),
    unit: bcs.string(),
    subunit: bcs.string(),
    decimals: bcs.u8(),
    useMetricPrefix: bcs.bool(),
    coinType: CoinType,
    totalSupply: bcs.u64(),
});

export const IotajsonrpcBigInt = bcs.struct('IotajsonrpcBigInt', {
    int: bcs.u256(),
});

export const ParametersProtocol = bcs.struct('ParametersProtocol', {
    epoch: IotajsonrpcBigInt,
    protocolVersion: IotajsonrpcBigInt,
    systemStateVersion: IotajsonrpcBigInt,
    iotaTotalSupply: IotajsonrpcBigInt,
    referenceGasPrice: IotajsonrpcBigInt,
    epochStartTimestampMs: IotajsonrpcBigInt,
    epochDurationMs: IotajsonrpcBigInt,
});

export const ParametersL1Params = bcs.struct('ParametersL1Params', {
    protocol: ParametersProtocol,
    baseToken: ParametersBaseToken,
});

export const BlocklogBlockInfo = bcs.struct('BlocklogBlockInfo', {
    schemaVersion: bcs.u8(),
    blockIndex: bcs.u32(),
    timestamp: bcs.u64(),
    previousAnchor: IscStateAnchor,
    l1Params: ParametersL1Params,
    totalRequests: bcs.u16(),
    numSuccessfulRequests: bcs.u16(),
    numOffLedgerRequests: bcs.u16(),
    gasBurned: bcs.u64(),
    gasFeeCharged: bcs.u64(),
});

export const IscCallTarget = bcs.struct('IscCallTarget', {
    contract: bcs.u32(),
    entryPoint: bcs.u32(),
});

export const IscMessage = bcs.struct('IscMessage', {
    target: IscCallTarget,
    params: bcs.vector(bcs.vector(bcs.u8())),
});

export const CryptolibPublicKey = bcs.struct('CryptolibPublicKey', {
    key: bcs.vector(bcs.u8()),
});

export const CryptolibSignature = bcs.struct('CryptolibSignature', {
    signatureScheme: bcs.u8(),
    publicKey: CryptolibPublicKey,
    signature: bcs.fixedArray(64, bcs.u8()),
});

export const RootContractRecord = bcs.struct('RootContractRecord', {
    name: bcs.string(),
});

export const LoTuple2_Hname_ContractRecord = bcs.struct('LoTuple2_Hname_ContractRecord', {
    a: bcs.u32(),
    b: RootContractRecord,
});

export const IscContractAgentID = bcs.struct('IscContractAgentID', {
    hname: bcs.u32(),
});

export const IscEthereumAddressAgentID = bcs.struct('IscEthereumAddressAgentID', {
    eth: bcs.fixedArray(20, bcs.u8()),
});

export const IscNilAgentID = bcs.struct('IscNilAgentID', {});

export const IscAddressAgentID = bcs.struct('IscAddressAgentID', {
    a: bcs.fixedArray(32, bcs.u8()),
});

export const IscAgentID = bcs.enum('IscAgentID', {
    NoType: null,
    AddressAgentID: IscAddressAgentID,
    ContractAgentID: IscContractAgentID,
    EthereumAddressAgentID: IscEthereumAddressAgentID,
    NilAgentID: IscNilAgentID,
});

export const balanceInputs = bcs.struct('balanceInputs', {
    agentID: bcs.option(IscAgentID),
});

export const balanceOutputs = bcs.struct('balanceOutputs', {
    coinBalances: bcs.map(CoinType, bcs.u64()),
});

export const balanceBaseTokenEVMInputs = bcs.struct('balanceBaseTokenEVMInputs', {
    agentID: bcs.option(IscAgentID),
});

export const balanceBaseTokenEVMOutputs = bcs.struct('balanceBaseTokenEVMOutputs', {
    evmBaseTokenBalance: bcs.u256(),
});

export const AtomicnoCopy = bcs.struct('AtomicnoCopy', {});

export const Atomicalign64 = bcs.struct('Atomicalign64', {});

export const IscEvent = bcs.struct('IscEvent', {
    contractID: bcs.u32(),
    topic: bcs.string(),
    timestamp: bcs.u64(),
    payload: bcs.vector(bcs.u8()),
});

export const UtilRatio32 = bcs.struct('UtilRatio32', {
    a: bcs.u32(),
    b: bcs.u32(),
});

export const GasFeePolicy = bcs.struct('GasFeePolicy', {
    eVMGasRatio: UtilRatio32,
    gasPerToken: UtilRatio32,
    validatorFeeShare: bcs.u8(),
});

export const GasLimits = bcs.struct('GasLimits', {
    maxGasPerBlock: bcs.u64(),
    minGasPerRequest: bcs.u64(),
    maxGasPerRequest: bcs.u64(),
    maxGasExternalViewCall: bcs.u64(),
});

export const IscPublicChainMetadata = bcs.struct('IscPublicChainMetadata', {
    eVMJsonRPCURL: bcs.string(),
    eVMWebSocketURL: bcs.string(),
    name: bcs.string(),
    description: bcs.string(),
    website: bcs.string(),
});

export const IscChainInfo = bcs.struct('IscChainInfo', {
    chainID: bcs.fixedArray(32, bcs.u8()),
    chainOwnerID: IscAgentID,
    gasFeePolicy: GasFeePolicy,
    gasLimits: GasLimits,
    blockKeepAmount: bcs.bytes(4),
    publicURL: bcs.string(),
    metadata: IscPublicChainMetadata,
});

export const IscVMErrorCode = bcs.struct('IscVMErrorCode', {
    contractID: bcs.u32(),
    iD: bcs.u16(),
});

export const GasBurnRecord = bcs.struct('GasBurnRecord', {
    code: bcs.u16(),
    gasBurned: bcs.u64(),
});

export const IscVMErrorParam = bcs.enum('IscVMErrorParam', {
    NoType: null,
    uint16: bcs.u16(),
    int64: bcs.bytes(8),
    string: bcs.string(),
    uint8: bcs.u8(),
    int32: bcs.bytes(4),
    uint64: bcs.u64(),
    int16: bcs.bytes(2),
    uint32: bcs.u32(),
});

export const IscUnresolvedVMError = bcs.struct('IscUnresolvedVMError', {
    errorCode: IscVMErrorCode,
    params: bcs.vector(IscVMErrorParam),
});

export const GasBurnLog = bcs.struct('GasBurnLog', {
    records: bcs.vector(GasBurnRecord),
});

export const TypesAccessTuple = bcs.struct('TypesAccessTuple', {
    address: bcs.fixedArray(20, bcs.u8()),
    storageKeys: bcs.vector(bcs.fixedArray(32, bcs.u8())),
});
