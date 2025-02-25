use bcs;
use hex;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Pizza {
    olive_oils: u16,
    yeast: u16,
    flour: u16,
    water: u16,
    salt: u16,
    tomato_sauce: u16,
    cheese: u16,
    pineapple: u16,
}

fn main() {
    // Convert the hex string into a byte vector.
    let hex_str = "0a000300620272011200c800b4000000";
    let data = hex::decode(hex_str).expect("Decoding failed");

    // Deserialize the bytes back into a Pizza struct.
    let pizza: Pizza = bcs::from_bytes(&data).expect("Deserialization failed");

    println!("Expected Pizza Struct: {:?}", pizza);
}
