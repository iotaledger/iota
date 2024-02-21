// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from "vitest";
import { deserialize, serialize, update_identifiers } from "../pkg";

describe("MBF-Wasm De/Serialization", () => {
  it("should de / ser", () => {
    let patched = update_identifiers(pokemonBytes(), {
        'Stats': 'PokeStats', 
        'pokemon_v1': 'capymon',
        'new': 'capy_new',
        'speed': 'capy_speed',
    });

    expect(serialize(deserialize(patched))).toEqual(patched);
  });
});

// pre-compiled Move module;
function templateBytes() {
  return "a11ceb0b060000000901000a020a10031a1b043504053924075d860108e301400aa3020b0cae021a000c0109010a010e010f0001020000000c00010304000402020000070001000108040500020403010102030b0901010c040d06070002020308020800070803000108000209000708030107080301080201060803010501080102090005044974656d085245474953545259095478436f6e74657874035549440e636c61696d5f616e645f6b6565700b64756d6d795f6669656c6402696404696e6974036e6577066f626a656374077061636b6167650f7075626c69635f7472616e736665720872656769737472790673656e646572087472616e736665720a74785f636f6e7465787400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000201050101020106080200000000010b0b000a0138000a01110112010b012e110438010200";
}

function pokemonBytes() {
    return "a11ceb0b060000000a01000202020403064b055139078a019b0108a5022006c5021e0ae302140cf702f7030dee0610000a000007000009000100000d00010000020201000008030400000b050100000506010000010607000004060700000c060700000e060700000f06070000060607000010060800000309050000070a050004060800060800020201030603030303030308020202020202020a0201080000010608000102010a02020708000301070800010104030303030553746174730661747461636b0664616d6167650b64656372656173655f687007646566656e7365026870056c6576656c086c6576656c5f7570036e65770f706879736963616c5f64616d6167650a706f6b656d6f6e5f7631077363616c696e670e7370656369616c5f61747461636b0e7370656369616c5f64616d6167650f7370656369616c5f646566656e73650573706565640574797065730000000000000000000000000000000000000000000000000000000000000000030800ca9a3b0000000003080000000000000000030801000000000000000002080503010204020c020e020f020602100a02000100000b320a0331d92604090a0331ff250c04050b090c040b04040e05140b01010b00010701270a023100240419051f0b01010b00010702270a00100014340b00100114340b01100214340b02340b03340700110202010100000b320a0331d92604090a0331ff250c04050b090c040b04040e05140b01010b00010701270a023100240419051f0b01010b00010702270a00100014340b00100314340b01100414340b02340b03340700110202020000000c2a0602000000000000000b0018060100000000000000180605000000000000001a060200000000000000160c070a050b01180b021a0c060b070b03180b06180632000000000000001a0602000000000000000a0518160c080a050b041806ff000000000000001a0c090b080b0918060100000000000000180b051a0203010000050d0b00340700180b010b020b030b040b050b060b071200020401000005020700020501000005040b00100514020601000005040b00100114020701000005040b00100214020801000005040b00100314020901000005040b00100414020a01000005040b00100614020b01000005040b00100014020c01000005040b00100714020d01000005140a010a0010051424040b0600000000000000000b000f051505130a001005140b01170b000f0515020e01000005090a001000143101160b000f0015020006000100020003000400000005000700";
}
