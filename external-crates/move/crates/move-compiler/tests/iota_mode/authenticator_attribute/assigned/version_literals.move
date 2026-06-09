module 0x42::M {
  #[authenticator = 3]
  public fun authenticator_function_inferred() {}

  #[authenticator = 3u8]
  public fun authenticator_function_u8() {}

  #[authenticator = 3u16]
  public fun authenticator_function_u16() {}

  #[authenticator = 3u32]
  public fun authenticator_function_u32() {}

  #[authenticator = 3u64]
  public fun authenticator_function_u64() {}

  #[authenticator = 3u128]
  public fun authenticator_function_u128() {}

  #[authenticator = 3u256]
  public fun authenticator_function_u256() {}
}