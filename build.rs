fn main() {
    println!("cargo:rerun-if-changed=data/embeddings.bin");
}
