fn main() {
    tonic_build::compile_protos("proto/checkpoint.proto").unwrap();
}
