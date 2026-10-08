// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Calls to the groth16 native functions of the IOTA framework
//! (`0x2::groth16`). The keys and proofs are the framework's own test vectors,
//! so no package has to be published.

use fastcrypto::encoding::{Encoding, Hex};
use iota_sdk_types::{Argument, Identifier};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, programmable_transaction_builder::ProgrammableTransactionBuilder,
};

use crate::command::{Groth16Curve, Groth16Function};

/// `test_verify_groth_16_proof_bn254` in the framework's `groth16_tests.move`:
/// the four parts of a prepared verifying key, one public input, and a proof
/// that verifies against them.
const BN254_VERIFY: [&str; 6] = [
    // vk_gamma_abc_g1
    concat!(
        "e8324a3242be5193eb38cca8761691ce061e89ce86f1fce8fd7ef40808f12da3c67d9ed5667c841f",
        "956e11adbbe240ddf37a1e3a4a890600dc88f608b897898e",
    ),
    // alpha_g1_beta_g2
    concat!(
        "51e6d72cd3b0914dd232653f84e7971d3e5bbcde6b47ff8d6c05277e579f1c1eb2fe30aa252c6395",
        "0de6ea00dd21a1027f6d130357e47c31fafeca0d31e19406231df42bc11ce376f8cf75135d9074f0",
        "81c242c31f198d151ec69ec37d67cc2b12542cb306a7823c8b194f13672176c6ee8266b2a0c9f57a",
        "5dbdb2278046b511d44e715a3ebe02ec2e1cf493c1b1ada84676e234134a6da5a552f61d4e905e15",
        "c0dc58a3414d74304775de5ba8571128f3548d269b51fdc08d5b646fd9157e0a2bc0c4bec5a9a604",
        "8d17d1d6cd941b4d459f1de0c7c1d417f33995d2a8dd670b91f0baaccaaf2802100901711885026a",
        "5ec97fbbb801000d0d01185651947c1900e336921d07eb16d0e25a2192829540ad5eeb1c498ba9c6",
        "316e16807a55dc2b9a7f3dea2e4a2f485ed1295a96d6ca86851842b3a22f83507f93ac66a1dc341d",
        "5d22f592527d8ea5c12db16bbabe24b76b3e1baf825c8dcf147be369fd8c5300fd77d0aa8dce730e",
        "4e7442c93c4890023f3a266c9fbc90ebbf72825e798c4c00",
    ),
    // gamma_g2_neg_pc
    concat!(
        "240a80664919b9f7490209cff12bfd81c32c272607dc004661c792082cbe282ef826f56a3822ebd7",
        "2345f86c7ee9872e23f10d1f2dbf43f8aca5dc2ceb5388a5",
    ),
    // delta_g2_neg_pc
    concat!(
        "f755df8c90edab48ac5adafef6a5a461902217f392e3aa4c34c0462b700c18164f79018778755980",
        "d491647de11ecc51fda2cc17171c4b44485ec37ccd23a69b",
    ),
    // public input
    "3fd7c445c6845a9399d1a7b8394c16373399a037786c169f16219359d3be840a",
    // proof points
    concat!(
        "dd2ef02e57d6a282df6b7f36c134ab7e55c2e04c5b8cbd7831be18e0e7224623ae8bd6c41637c10c",
        "bd02f5e68de6394461f417895ddd264d6f0ddacf68c6cd02feb8881f0efa599139a6faf4223dd874",
        "3777c4346cba52322eb466af96f2be9f813af1450f84d6f8029804f60cac1add70ad1a3d4226404f",
        "84f4022dc18caa0f",
    ),
];

/// Verifying key from `test_prepare_verifying_key_bn254` in the framework's
/// `groth16_tests.move`.
const BN254_VERIFYING_KEY: &str = concat!(
    "53d75f472c207c7fcf6a34bc1e50cf0d7d2f983dd2230ffcaf280362d162c3871cae3e4f91b77ead",
    "aac316fe625e3764fb39af2bb5aa25007e9bc6b116f6f02f597ad7c28c4a33da5356e656dcef4660",
    "d7375973fe0d7b6dc642d51f16b6c8806030ca5b462a3502d560df7ff62b7f1215195233f688320d",
    "e19e4b3a2a2cb6120ae49bcc0abbd3cbbf06b29b489edbf86e3b679f4e247464992145f468e3c08d",
    "b41e5e09002a7170cb4cc56ae96b152d17b6b0d1b9333b41f2325c3c8a9d2e2df98f8e2315884fae",
    "52b3c6bb329df0359daac4eff4d2e7ce729078b10d79d42f02000000000000001dcc52e058148a62",
    "2c51acfdee6e181252ec0e9717653f0be1faaf2a68222e0dd2ccf4e1e8b088efccfdb955a1ff4a0f",
    "d28ae2ccbe1a112449ddae8738fb40b0",
);

/// `test_verify_groth_16_proof_bls12381` in the framework's
/// `groth16_tests.move`: the four parts of a prepared verifying key, one public
/// input, and a proof that verifies against them.
const BLS12381_VERIFY: [&str; 6] = [
    // vk_gamma_abc_g1
    concat!(
        "ada3c24e8c2e63579cc03fd1f112a093a17fc8ab0ff6eee7e04cab7bf8e03e7645381f309ec11330",
        "9e05ac404c77ac7c8585d5e4328594f5a70a81f6bd4f29073883ee18fd90e2aa45d0fc7376e81e2f",
        "df5351200386f5732e58eb6ff4d318dc",
    ),
    // alpha_g1_beta_g2
    concat!(
        "8b0f85a9e7d929244b0af9a35af10717bd667b6227aae37a6d336e815fb0d850873e0d87968345a4",
        "93b2d31aa8aa400d9820af1d35fa862d1b339ea1f98ac70db7faa304bff120a151a1741d782d08b8",
        "f1c1080d4d2f3ebee63ac6cadc666605be306de0973be38fbbf0f54b476bbb002a74ff9506a2b9b9",
        "a34b99bfa7481a84a2c9face7065c19d7069cc5738c5350b886a5eeebe656499d2ffb360afc7aff2",
        "0fa9ee689fb8b46863e90c85224e8f597bf323ad4efb02ee96eb40221fc89918a2c740eabd288647",
        "6c7f247a3eb34f0106b3b51cf040e2cdcafea68b0d8eecabf58b5aa2ece3d86259cf2dfa3efab117",
        "0c6eb11948826def533849b68335d76d60f3e16bb5c629b1c24df2bdd1a7f13c754d7fe38617ecd7",
        "783504e4615e5c13168185cc08de8d63a0f7032ab7e82ff78cf0bc46a84c98f2d95bb5af355cbbe5",
        "25c44d5c1549c169dfe119a219dbf9038ec73729d187bd0e3ed369e4a2ec2be837f3dcfd958aea71",
        "10627d2c0192d262f17e722509c17196005b646a556cf010ef9bd2a2a9b937516a5ecdee516e77d1",
        "4278e96bc891b630fc833dda714343554ae127c49460416430b7d4f048d08618058335dec0728ad3",
        "7d10dd9d859c385a38673e71cc98e8439da0accc29de5c92d3c3dc98e199361e9f7558e8b0a2a315",
        "ccc5a72f54551f07fad6f6f4615af498aba98aea01a13a4eb84667fd87ee9782b1d812a03f8814f0",
        "42823a7701238d0fec1e7dec2a26ffea00330b5c7930e95138381435d2a59f51313a48624e30b0a6",
        "85e357874d41a0a19d83f7420c1d9c04",
    ),
    // gamma_g2_neg_pc
    concat!(
        "b675d1ff988116d1f2965d3c0c373569b74d0a1762ea7c4f4635faa5b5a8fa198a2a2ce6153f390a",
        "658dc9ad01a415491747e9de7d5f493f59cf05a52eb46eaac397ffc47aef1396cf0d8b75d0664077",
        "ea328ad6b63284b42972a8f11c523a60",
    ),
    // delta_g2_neg_pc
    concat!(
        "8229cb9443ef1fb72887f917f500e2aef998717d91857bcb92061ecd74d1d24c2b2b282736e8074e",
        "4316939b4c9853c117aa08ed49206860d648818b2cccb526585f5790161b1730d39c73603b482424",
        "a27bba891aaa6d99f3025d3df2a6bd42",
    ),
    // public input
    "440758042e68b76a376f2fecf3a5a8105edb194c3e774e5a760140305aec8849",
    // proof points
    concat!(
        "a29981304df8e0f50750b558d4de59dbc8329634b81c986e28e9fff2b0faa52333b14a1f7b275b02",
        "9e13499d1f5dd8ab955cf5fa3000a097920180381a238ce12df52207597eade4a365a6872c0a19a3",
        "9c08a9bfb98b69a15615f90cc32660180ca32e565c01a49b505dd277713b1eae834df49643291a36",
        "01b11f56957bde02d5446406d0e4745d1bd32c8ccb8d8e80b877712f5f373016d2ecdeebb58caebc",
        "7a425b8137ebb1bd0c5b81c1d48151b25f0f24fe9602ba4e403811fb17db6f14",
    ),
];

/// Verifying key from `test_prepare_verifying_key_bls12381` in the framework's
/// `groth16_tests.move`.
const BLS12381_VERIFYING_KEY: &str = concat!(
    "a84d039ad1ae98eeeee4c8ba9af9b6c5d1cfcb98c3fc92ccfcebd77bcccffa1d170d39da29e9b4aa",
    "83b98680cb90bb25946b2b70f9e3565510c5361d5d65cb458a0b3177d612dd340b8f8f8493c27724",
    "54e3e8f577a3f77865df851d1a159b800c2ec5bae889029fc419678e83dee900465d60e7ef26f614",
    "940e719c6f7c0c7db57464fa0481a93c18d52cb2fbf8dcf0a398b153643614fc1071a54e288edb64",
    "02f1d9e00d3408c76d95c16885cc992dff5c6ebee3b739cb22359ab2d126026a1626c43ea7b898a7",
    "c1d2904c1bd4bbce5d0b1b16fab8535a52d1b08a5217df2e912ee1b0f4140892afa31d479f78dfbc",
    "82ab58a209ad00df6c86ab14841e8daa7a380a6853f28bacf38aad9903b6149fff4b119dea16de8a",
    "a3e5050b9d563a01009e061a950c233f66511c8fae2a8c58503059821df7f6defbba8f93d26e412c",
    "c07b66a9f3cdd740cce5c8488ce94fc8020000000000000081aabea18713222ac45a6ef3208a09f5",
    "5ce2dde8a11cc4b12788be2ae77ae318176d631d36d80942df576af651b57a31a95f2e9bcaebbb53",
    "a588251634715599f7a7e9d51fe872fe312edf0b39d98f0d7f8b5554f96f759c041ea38b4b1e5e19",
);

/// Name of the `0x2::groth16` function that returns `curve`.
fn curve_function_name(curve: Groth16Curve) -> &'static str {
    match curve {
        Groth16Curve::Bn254 => "bn254",
        Groth16Curve::Bls12381 => "bls12381",
    }
}

/// Adds `calls` calls to `function` on `curve` to the transaction.
pub(crate) fn add_groth16_calls(
    builder: &mut ProgrammableTransactionBuilder,
    curve: Groth16Curve,
    function: Groth16Function,
    calls: u32,
) {
    let curve_arg = groth16_call(builder, curve_function_name(curve), vec![]);
    match function {
        Groth16Function::Verify => {
            let [
                vk_gamma_abc_g1,
                alpha_g1_beta_g2,
                gamma_g2_neg_pc,
                delta_g2_neg_pc,
                public_inputs,
                proof_points,
            ] = match curve {
                Groth16Curve::Bn254 => BN254_VERIFY,
                Groth16Curve::Bls12381 => BLS12381_VERIFY,
            }
            .map(|hex| pure_bytes(builder, hex));
            let pvk = groth16_call(
                builder,
                "pvk_from_bytes",
                vec![
                    vk_gamma_abc_g1,
                    alpha_g1_beta_g2,
                    gamma_g2_neg_pc,
                    delta_g2_neg_pc,
                ],
            );
            let inputs = groth16_call(
                builder,
                "public_proof_inputs_from_bytes",
                vec![public_inputs],
            );
            let proof = groth16_call(builder, "proof_points_from_bytes", vec![proof_points]);
            for _ in 0..calls {
                groth16_call(
                    builder,
                    "verify_groth16_proof",
                    vec![curve_arg, pvk, inputs, proof],
                );
            }
        }
        Groth16Function::Prepare => {
            let verifying_key = pure_bytes(
                builder,
                match curve {
                    Groth16Curve::Bn254 => BN254_VERIFYING_KEY,
                    Groth16Curve::Bls12381 => BLS12381_VERIFYING_KEY,
                },
            );
            for _ in 0..calls {
                groth16_call(
                    builder,
                    "prepare_verifying_key",
                    vec![curve_arg, verifying_key],
                );
            }
        }
    }
}

/// Adds the hex-encoded test vector `hex` as a pure `vector<u8>` input.
fn pure_bytes(builder: &mut ProgrammableTransactionBuilder, hex: &str) -> Argument {
    let bytes = Hex::decode(hex).expect("groth16 test vectors are valid hex");
    builder
        .pure(bytes)
        .expect("a byte vector always serializes")
}

/// Adds a call to `function` of `0x2::groth16`. An argument given to a
/// reference parameter is only borrowed, so one result can go to many calls.
fn groth16_call(
    builder: &mut ProgrammableTransactionBuilder,
    function: &str,
    arguments: Vec<Argument>,
) -> Argument {
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::new("groth16").unwrap(),
        Identifier::new(function).unwrap(),
        vec![],
        arguments,
    )
}
