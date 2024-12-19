//# init --edition 2024.beta

//# publish
module 0x42::m {

    public enum Temperature {
        Fahrenheit(u64),
        Celsius(u32)
    }

    public fun f(): Temperature {
        Temperature::Fahrenheit(32)
    }

    public fun c(): Temperature {
        Temperature::Celsius(0)
    }

    public fun dtor(t: Temperature) {
        match (t) {
            Temperature::Fahrenheit(_) => (), 
            Temperature::Celsius(_) => (), 
        }
    }

    public fun is_temperature_fahrenheit(t: &Temperature): bool {
       match (t) {
          Temperature::Fahrenheit(_) => true,
          _ => false,
       }
    }
}

//# run
module 0x43::main {
    use 0x42::m;
    fun main() {
        let f = m::f();
        let c = m::c();
        assert!(f.is_temperature_fahrenheit());
        assert!(!c.is_temperature_fahrenheit());

        m::dtor(f);
        m::dtor(c);
    }
}
