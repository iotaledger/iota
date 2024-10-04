module a::a {

    public enum Temperature {
        Fahrenheit(u64),
        Celsius(u32)
    }

    fun is_temperature_fahrenheit(t: &Temperature): bool {
       match (t) {
          Temperature::Fahrenheit(_) => true,
          _ => false,
       }
    }

}
