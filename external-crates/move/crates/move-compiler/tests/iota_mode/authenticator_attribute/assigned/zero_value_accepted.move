module 0x42::M {
  // The value of zero is incorrect, but the compiler does not know about this.
  // The verifier must handle whether or not the authenticator is within the accepted limits.
  #[authenticator = 0]
  public fun authenticator_function() {}
}