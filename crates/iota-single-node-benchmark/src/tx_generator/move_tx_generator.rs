// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use fastcrypto::{
    encoding::{Encoding, Hex},
    hash::Sha256,
    traits::{KeyPair, Signer},
    vrf::VRFKeyPair,
};
use iota_sdk_types::{
    Address, Argument, Identifier, ObjectId, ObjectReference, SharedObjectReference, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, DEFAULT_VALIDATOR_GAS_PRICE, TransactionEnvelope},
};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    command::{Groth16Curve, GroupOp, HashFamily, PtbParams, SigScheme},
    mock_account::Account,
    tx_generator::TxGenerator,
};

/// A valid (signature, public key, message) triple for the configured
/// scheme, generated once and passed to the Move verify loop as pure
/// arguments. The Move side asserts the verification result, so an invalid
/// fixture aborts the transaction instead of silently measuring the
/// cheaper failure path.
struct SigFixture {
    signature: Vec<u8>,
    public_key: Vec<u8>,
    msg: Vec<u8>,
}

impl SigFixture {
    fn generate(scheme: SigScheme, msg_size: u64) -> Self {
        // Fixed seed: identical fixtures across runs and machines.
        let mut rng = StdRng::from_seed([42; 32]);
        let msg = vec![7u8; msg_size as usize];
        match scheme {
            SigScheme::Ed25519 => {
                let kp = fastcrypto::ed25519::Ed25519KeyPair::generate(&mut rng);
                let signature = kp.sign(&msg);
                Self {
                    signature: signature.as_ref().to_vec(),
                    public_key: kp.public().as_ref().to_vec(),
                    msg,
                }
            }
            SigScheme::BlsMinSig => {
                let kp = fastcrypto::bls12381::min_sig::BLS12381KeyPair::generate(&mut rng);
                let signature = kp.sign(&msg);
                Self {
                    signature: signature.as_ref().to_vec(),
                    public_key: kp.public().as_ref().to_vec(),
                    msg,
                }
            }
            SigScheme::BlsMinPk => {
                let kp = fastcrypto::bls12381::min_pk::BLS12381KeyPair::generate(&mut rng);
                let signature = kp.sign(&msg);
                Self {
                    signature: signature.as_ref().to_vec(),
                    public_key: kp.public().as_ref().to_vec(),
                    msg,
                }
            }
            // The secp schemes sign the sha256 of the message, matching the
            // hash selector the Move verify loops pass to the native.
            SigScheme::Secp256k1 => {
                let kp = fastcrypto::secp256k1::Secp256k1KeyPair::generate(&mut rng);
                let signature = kp.sign_with_hash::<Sha256>(&msg);
                Self {
                    signature: signature.as_ref().to_vec(),
                    public_key: kp.public().as_ref().to_vec(),
                    msg,
                }
            }
            SigScheme::Secp256r1 => {
                let kp = fastcrypto::secp256r1::Secp256r1KeyPair::generate(&mut rng);
                let signature = kp.sign_with_hash::<Sha256>(&msg);
                Self {
                    signature: signature.as_ref().to_vec(),
                    public_key: kp.public().as_ref().to_vec(),
                    msg,
                }
            }
        }
    }

    fn move_function(scheme: SigScheme) -> &'static str {
        match scheme {
            SigScheme::Ed25519 => "ed25519_verify_loop",
            SigScheme::BlsMinSig => "bls12381_min_sig_verify_loop",
            SigScheme::BlsMinPk => "bls12381_min_pk_verify_loop",
            SigScheme::Secp256k1 => "secp256k1_verify_loop",
            SigScheme::Secp256r1 => "secp256r1_verify_loop",
        }
    }
}

/// A valid ECVRF (output hash, alpha string, public key, proof) tuple,
/// generated once; the public key and proof are BCS-encoded the way the
/// native deserializes them. The Move loop asserts the verification result.
struct EcvrfFixture {
    hash: Vec<u8>,
    alpha: Vec<u8>,
    public_key: Vec<u8>,
    proof: Vec<u8>,
}

impl EcvrfFixture {
    fn generate(alpha_size: u64) -> Self {
        // Fixed seed: identical fixtures across runs and machines.
        let mut rng = StdRng::from_seed([43; 32]);
        let kp = fastcrypto::vrf::ecvrf::ECVRFKeyPair::generate(&mut rng);
        let alpha = vec![7u8; alpha_size as usize];
        let (hash, proof) = kp.output(&alpha);
        Self {
            hash: hash.to_vec(),
            alpha,
            public_key: bcs::to_bytes(&kp.pk).unwrap(),
            proof: bcs::to_bytes(&proof).unwrap(),
        }
    }
}

/// A known-good Groth16 (prepared verifying key, public inputs, proof)
/// fixture per curve, copied verbatim from the framework's groth16 unit
/// tests. Proof generation needs a circuit, so unlike the signature
/// fixtures these cannot be generated at setup.
struct Groth16Fixture {
    curve_id: u8,
    vk: Vec<u8>,
    alpha: Vec<u8>,
    gamma: Vec<u8>,
    delta: Vec<u8>,
    inputs: Vec<u8>,
    proof: Vec<u8>,
}

impl Groth16Fixture {
    fn for_curve(curve: Groth16Curve) -> Self {
        let hex = |s: &str| Hex::decode(s).unwrap();
        match curve {
            Groth16Curve::Bls12381 => Self {
                curve_id: 0,
                vk: hex(
                    "ada3c24e8c2e63579cc03fd1f112a093a17fc8ab0ff6eee7e04cab7bf8e03e7645381f309ec113309e05ac404c77ac7c8585d5e4328594f5a70a81f6bd4f29073883ee18fd90e2aa45d0fc7376e81e2fdf5351200386f5732e58eb6ff4d318dc",
                ),
                alpha: hex(
                    "8b0f85a9e7d929244b0af9a35af10717bd667b6227aae37a6d336e815fb0d850873e0d87968345a493b2d31aa8aa400d9820af1d35fa862d1b339ea1f98ac70db7faa304bff120a151a1741d782d08b8f1c1080d4d2f3ebee63ac6cadc666605be306de0973be38fbbf0f54b476bbb002a74ff9506a2b9b9a34b99bfa7481a84a2c9face7065c19d7069cc5738c5350b886a5eeebe656499d2ffb360afc7aff20fa9ee689fb8b46863e90c85224e8f597bf323ad4efb02ee96eb40221fc89918a2c740eabd2886476c7f247a3eb34f0106b3b51cf040e2cdcafea68b0d8eecabf58b5aa2ece3d86259cf2dfa3efab1170c6eb11948826def533849b68335d76d60f3e16bb5c629b1c24df2bdd1a7f13c754d7fe38617ecd7783504e4615e5c13168185cc08de8d63a0f7032ab7e82ff78cf0bc46a84c98f2d95bb5af355cbbe525c44d5c1549c169dfe119a219dbf9038ec73729d187bd0e3ed369e4a2ec2be837f3dcfd958aea7110627d2c0192d262f17e722509c17196005b646a556cf010ef9bd2a2a9b937516a5ecdee516e77d14278e96bc891b630fc833dda714343554ae127c49460416430b7d4f048d08618058335dec0728ad37d10dd9d859c385a38673e71cc98e8439da0accc29de5c92d3c3dc98e199361e9f7558e8b0a2a315ccc5a72f54551f07fad6f6f4615af498aba98aea01a13a4eb84667fd87ee9782b1d812a03f8814f042823a7701238d0fec1e7dec2a26ffea00330b5c7930e95138381435d2a59f51313a48624e30b0a685e357874d41a0a19d83f7420c1d9c04",
                ),
                gamma: hex(
                    "b675d1ff988116d1f2965d3c0c373569b74d0a1762ea7c4f4635faa5b5a8fa198a2a2ce6153f390a658dc9ad01a415491747e9de7d5f493f59cf05a52eb46eaac397ffc47aef1396cf0d8b75d0664077ea328ad6b63284b42972a8f11c523a60",
                ),
                delta: hex(
                    "8229cb9443ef1fb72887f917f500e2aef998717d91857bcb92061ecd74d1d24c2b2b282736e8074e4316939b4c9853c117aa08ed49206860d648818b2cccb526585f5790161b1730d39c73603b482424a27bba891aaa6d99f3025d3df2a6bd42",
                ),
                inputs: hex("440758042e68b76a376f2fecf3a5a8105edb194c3e774e5a760140305aec8849"),
                proof: hex(
                    "a29981304df8e0f50750b558d4de59dbc8329634b81c986e28e9fff2b0faa52333b14a1f7b275b029e13499d1f5dd8ab955cf5fa3000a097920180381a238ce12df52207597eade4a365a6872c0a19a39c08a9bfb98b69a15615f90cc32660180ca32e565c01a49b505dd277713b1eae834df49643291a3601b11f56957bde02d5446406d0e4745d1bd32c8ccb8d8e80b877712f5f373016d2ecdeebb58caebc7a425b8137ebb1bd0c5b81c1d48151b25f0f24fe9602ba4e403811fb17db6f14",
                ),
            },
            Groth16Curve::Bn254 => Self {
                curve_id: 1,
                vk: hex(
                    "e8324a3242be5193eb38cca8761691ce061e89ce86f1fce8fd7ef40808f12da3c67d9ed5667c841f956e11adbbe240ddf37a1e3a4a890600dc88f608b897898e",
                ),
                alpha: hex(
                    "51e6d72cd3b0914dd232653f84e7971d3e5bbcde6b47ff8d6c05277e579f1c1eb2fe30aa252c63950de6ea00dd21a1027f6d130357e47c31fafeca0d31e19406231df42bc11ce376f8cf75135d9074f081c242c31f198d151ec69ec37d67cc2b12542cb306a7823c8b194f13672176c6ee8266b2a0c9f57a5dbdb2278046b511d44e715a3ebe02ec2e1cf493c1b1ada84676e234134a6da5a552f61d4e905e15c0dc58a3414d74304775de5ba8571128f3548d269b51fdc08d5b646fd9157e0a2bc0c4bec5a9a6048d17d1d6cd941b4d459f1de0c7c1d417f33995d2a8dd670b91f0baaccaaf2802100901711885026a5ec97fbbb801000d0d01185651947c1900e336921d07eb16d0e25a2192829540ad5eeb1c498ba9c6316e16807a55dc2b9a7f3dea2e4a2f485ed1295a96d6ca86851842b3a22f83507f93ac66a1dc341d5d22f592527d8ea5c12db16bbabe24b76b3e1baf825c8dcf147be369fd8c5300fd77d0aa8dce730e4e7442c93c4890023f3a266c9fbc90ebbf72825e798c4c00",
                ),
                gamma: hex(
                    "240a80664919b9f7490209cff12bfd81c32c272607dc004661c792082cbe282ef826f56a3822ebd72345f86c7ee9872e23f10d1f2dbf43f8aca5dc2ceb5388a5",
                ),
                delta: hex(
                    "f755df8c90edab48ac5adafef6a5a461902217f392e3aa4c34c0462b700c18164f79018778755980d491647de11ecc51fda2cc17171c4b44485ec37ccd23a69b",
                ),
                inputs: hex("3fd7c445c6845a9399d1a7b8394c16373399a037786c169f16219359d3be840a"),
                proof: hex(
                    "dd2ef02e57d6a282df6b7f36c134ab7e55c2e04c5b8cbd7831be18e0e7224623ae8bd6c41637c10cbd02f5e68de6394461f417895ddd264d6f0ddacf68c6cd02feb8881f0efa599139a6faf4223dd8743777c4346cba52322eb466af96f2be9f813af1450f84d6f8029804f60cac1add70ad1a3d4226404f84f4022dc18caa0f",
                ),
            },
        }
    }
}

fn group_op_move_function(op: GroupOp) -> &'static str {
    match op {
        GroupOp::G1Add => "bls12381_g1_add_loop",
        GroupOp::G1Mul => "bls12381_g1_mul_loop",
        GroupOp::HashToG1 => "bls12381_hash_to_g1_loop",
        GroupOp::Pairing => "bls12381_pairing_loop",
    }
}

fn hash_move_function(family: HashFamily) -> &'static str {
    match family {
        HashFamily::Sha2_256 => "sha2_256_loop",
        HashFamily::Sha3_256 => "sha3_256_loop",
        HashFamily::Keccak256 => "keccak256_loop",
        HashFamily::Blake2b256 => "blake2b256_loop",
        HashFamily::HmacSha3_256 => "hmac_sha3_256_loop",
    }
}

pub struct MoveTxGenerator {
    move_package: ObjectId,
    params: PtbParams,
    root_objects: HashMap<Address, ObjectReference>,
    shared_objects: Vec<(ObjectId, Version)>,
    owned_objects: HashMap<Address, Vec<ObjectReference>>,
    generated_packages: Vec<ObjectId>,
    sig_fixture: Option<SigFixture>,
    ecvrf_fixture: Option<EcvrfFixture>,
    groth16_fixture: Option<Groth16Fixture>,
}

impl MoveTxGenerator {
    pub fn new(
        move_package: ObjectId,
        params: PtbParams,
        root_objects: HashMap<Address, ObjectReference>,
        shared_objects: Vec<(ObjectId, Version)>,
        owned_objects: HashMap<Address, Vec<ObjectReference>>,
        generated_packages: Vec<ObjectId>,
    ) -> Self {
        assert!(
            params.num_deletes <= params.num_dynamic_fields,
            "--num-deletes requires at least as many --num-dynamic-fields"
        );
        if params.num_packages_called > 0 {
            assert!(
                !generated_packages.is_empty(),
                "--num-packages-called requires generated packages"
            );
        }
        let sig_fixture = (params.num_sig_verifies > 0)
            .then(|| SigFixture::generate(params.sig_scheme, params.sig_msg_size));
        let ecvrf_fixture = (params.num_ecvrf_verifies > 0)
            .then(|| EcvrfFixture::generate(params.ecvrf_alpha_size));
        let groth16_fixture = (params.num_groth16_verifies > 0)
            .then(|| Groth16Fixture::for_curve(params.groth16_curve));
        Self {
            move_package,
            params,
            root_objects,
            shared_objects,
            owned_objects,
            generated_packages,
            sig_fixture,
            ecvrf_fixture,
            groth16_fixture,
        }
    }

    fn benchmark_call(
        &self,
        builder: &mut ProgrammableTransactionBuilder,
        function: &'static str,
        args: Vec<Argument>,
    ) {
        builder.programmable_move_call(
            self.move_package,
            Identifier::from_static("benchmark"),
            Identifier::from_static(function),
            vec![],
            args,
        );
    }
}

impl TxGenerator for MoveTxGenerator {
    fn generate_tx(&self, account: Account) -> TransactionEnvelope {
        let p = &self.params;
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            // Step 1: transfer `num_transfers` objects.
            // First object in the gas_objects is the gas object and we are not transferring
            // it.
            for i in 1..=p.num_transfers {
                let object = account.gas_objects[i as usize];
                if p.use_native_transfer {
                    builder.transfer_object(account.sender, object).unwrap();
                } else {
                    builder
                        .move_call(
                            self.move_package,
                            Identifier::from_static("benchmark"),
                            Identifier::from_static("transfer_coin"),
                            vec![],
                            vec![CallArg::ImmutableOrOwned(object)],
                        )
                        .unwrap();
                }
            }
            for shared_object in &self.shared_objects {
                builder
                    .move_call(
                        self.move_package,
                        Identifier::from_static("benchmark"),
                        Identifier::from_static("increment_shared_counter"),
                        vec![],
                        vec![CallArg::Shared(SharedObjectReference::new(
                            shared_object.0,
                            shared_object.1,
                            true,
                        ))],
                    )
                    .unwrap();
            }

            if !self.root_objects.is_empty() {
                // Step 2: Read all dynamic fields from the root object, then
                // optionally delete some of them.
                let root_object = self.root_objects.get(&account.sender).unwrap();
                let root_object_arg = builder
                    .obj(CallArg::ImmutableOrOwned(*root_object))
                    .unwrap();
                self.benchmark_call(&mut builder, "read_dynamic_fields", vec![root_object_arg]);
                if p.num_deletes > 0 {
                    let num_arg = builder.pure(p.num_deletes).unwrap();
                    self.benchmark_call(
                        &mut builder,
                        "delete_dynamic_fields",
                        vec![root_object_arg, num_arg],
                    );
                }
            }

            if p.computation > 0 {
                // Step 3: Run some computation.
                let computation_arg = builder.pure(p.computation as u64 * 100).unwrap();
                self.benchmark_call(&mut builder, "run_computation", vec![computation_arg]);
            }
            if p.num_mints > 0 {
                // Step 4: Mint some NFTs
                let mut contents = Vec::new();
                assert!(p.nft_size >= 32, "NFT size must be at least 32 bytes");
                for _ in 0..p.nft_size - 32 {
                    contents.push(7u8)
                }
                if p.use_batch_mint {
                    // create a vector of sender addresses to pass to batch_mint
                    let mut recipients = Vec::new();
                    for _ in 0..p.num_mints {
                        recipients.push(account.sender)
                    }
                    let args = vec![
                        builder.pure(recipients).unwrap(),
                        builder.pure(contents).unwrap(),
                    ];
                    self.benchmark_call(&mut builder, "batch_mint", args);
                } else {
                    // create PTB with a command that transfers each
                    for _ in 0..p.num_mints {
                        let args = vec![
                            builder.pure(account.sender).unwrap(),
                            builder.pure(contents.clone()).unwrap(),
                        ];
                        self.benchmark_call(&mut builder, "mint_one", args);
                    }
                }
            }

            // Interpreter cost-component shapes.
            if p.scalar_ops > 0 {
                let arg = builder.pure(p.scalar_ops).unwrap();
                self.benchmark_call(&mut builder, "scalar_arithmetic", vec![arg]);
            }
            if p.push_pop_ops > 0 {
                let arg = builder.pure(p.push_pop_ops).unwrap();
                self.benchmark_call(&mut builder, "push_pop", vec![arg]);
            }
            if p.vector_move_ops > 0 {
                let args = vec![
                    builder.pure(p.vector_move_ops).unwrap(),
                    builder.pure(p.vector_move_size).unwrap(),
                ];
                self.benchmark_call(&mut builder, "vector_move", args);
            }

            // Working-memory shapes.
            if p.locals_bytes > 0 {
                let arg = builder.pure(p.locals_bytes).unwrap();
                self.benchmark_call(&mut builder, "vector_in_locals", vec![arg]);
            }
            if p.tree_depth > 0 {
                let args = vec![
                    builder.pure(p.tree_depth).unwrap(),
                    builder.pure(p.tree_width).unwrap(),
                ];
                self.benchmark_call(&mut builder, "struct_tree", args);
            }

            // Native families.
            if p.num_hashes > 0 {
                let input = vec![7u8; p.hash_input_size as usize];
                let mut args = vec![builder.pure(p.num_hashes).unwrap()];
                if matches!(p.hash_family, HashFamily::HmacSha3_256) {
                    args.push(builder.pure(vec![11u8; 32]).unwrap());
                }
                args.push(builder.pure(input).unwrap());
                self.benchmark_call(&mut builder, hash_move_function(p.hash_family), args);
            }
            if let Some(fixture) = &self.sig_fixture {
                let args = vec![
                    builder.pure(p.num_sig_verifies).unwrap(),
                    builder.pure(fixture.signature.clone()).unwrap(),
                    builder.pure(fixture.public_key.clone()).unwrap(),
                    builder.pure(fixture.msg.clone()).unwrap(),
                ];
                self.benchmark_call(&mut builder, SigFixture::move_function(p.sig_scheme), args);
            }
            if let Some(fixture) = &self.ecvrf_fixture {
                let args = vec![
                    builder.pure(p.num_ecvrf_verifies).unwrap(),
                    builder.pure(fixture.hash.clone()).unwrap(),
                    builder.pure(fixture.alpha.clone()).unwrap(),
                    builder.pure(fixture.public_key.clone()).unwrap(),
                    builder.pure(fixture.proof.clone()).unwrap(),
                ];
                self.benchmark_call(&mut builder, "ecvrf_verify_loop", args);
            }
            if let Some(fixture) = &self.groth16_fixture {
                let args = vec![
                    builder.pure(p.num_groth16_verifies).unwrap(),
                    builder.pure(fixture.curve_id).unwrap(),
                    builder.pure(fixture.vk.clone()).unwrap(),
                    builder.pure(fixture.alpha.clone()).unwrap(),
                    builder.pure(fixture.gamma.clone()).unwrap(),
                    builder.pure(fixture.delta.clone()).unwrap(),
                    builder.pure(fixture.inputs.clone()).unwrap(),
                    builder.pure(fixture.proof.clone()).unwrap(),
                ];
                self.benchmark_call(&mut builder, "groth16_verify_loop", args);
            }
            if p.num_poseidon_hashes > 0 {
                let args = vec![
                    builder.pure(p.num_poseidon_hashes).unwrap(),
                    builder.pure(p.poseidon_input_count).unwrap(),
                ];
                self.benchmark_call(&mut builder, "poseidon_loop", args);
            }
            if p.num_group_ops > 0 {
                let mut args = vec![builder.pure(p.num_group_ops).unwrap()];
                if matches!(p.group_op, GroupOp::HashToG1) {
                    let msg = vec![7u8; p.group_op_input_size as usize];
                    args.push(builder.pure(msg).unwrap());
                }
                self.benchmark_call(&mut builder, group_op_move_function(p.group_op), args);
            }

            // Mutate-in-place and burn, on the setup-minted fixtures:
            // written objects with no creation, and tombstones with no
            // dynamic-field access.
            if p.num_mutations + p.num_burns > 0 {
                let fixtures = self
                    .owned_objects
                    .get(&account.sender)
                    .expect("owned-object fixtures were prepared at setup");
                let needed = (p.num_mutations + p.num_burns) as usize;
                assert!(fixtures.len() >= needed, "not enough owned-object fixtures");
                for oref in &fixtures[..p.num_mutations as usize] {
                    let arg = builder.obj(CallArg::ImmutableOrOwned(*oref)).unwrap();
                    self.benchmark_call(&mut builder, "mutate_object", vec![arg]);
                }
                for oref in &fixtures[p.num_mutations as usize..needed] {
                    let arg = builder.obj(CallArg::ImmutableOrOwned(*oref)).unwrap();
                    self.benchmark_call(&mut builder, "burn_object", vec![arg]);
                }
            }

            // Interpreter push-ratio shapes.
            if p.high_push_ops > 0 {
                let arg = builder.pure(p.high_push_ops).unwrap();
                self.benchmark_call(&mut builder, "high_push_ratio", vec![arg]);
            }
            if p.low_push_ops > 0 {
                let arg = builder.pure(p.low_push_ops).unwrap();
                self.benchmark_call(&mut builder, "low_push_ratio", vec![arg]);
            }

            // Calls into distinct generated packages: package-load count and
            // bytes vary independently of everything else.
            if p.num_packages_called > 0 {
                let len = self.generated_packages.len();
                let bytes: [u8; 8] = account.sender.as_bytes()[..8].try_into().unwrap();
                let start = (u64::from_le_bytes(bytes) as usize) % len;
                for i in 0..p.num_packages_called as usize {
                    let package = self.generated_packages[(start + i) % len];
                    builder.programmable_move_call(
                        package,
                        Identifier::from_static("generated"),
                        Identifier::from_static("load_me"),
                        vec![],
                        vec![],
                    );
                }
            }

            // Events.
            if p.num_events > 0 {
                let args = vec![
                    builder.pure(p.num_events).unwrap(),
                    builder.pure(p.event_size).unwrap(),
                ];
                self.benchmark_call(&mut builder, "emit_events", args);
            }

            builder.finish()
        };
        TestTransactionBuilder::new(
            account.sender,
            account.gas_objects[0],
            DEFAULT_VALIDATOR_GAS_PRICE,
        )
        .programmable(pt)
        .build_and_sign(account.private_key.as_ref())
    }

    fn name(&self) -> &'static str {
        "Programmable Move Transaction Generator"
    }
}
