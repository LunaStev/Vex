fn main() {
    println!(
        "cargo:rustc-env=VEX_HOST_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
