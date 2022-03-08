// build.rs

fn main() {
    cc::Build::new()
        .file("src/m68kcpu.c")
        .file("src/m68kops.c")
        .file("src/m68kdasm.c")
        .compile("musashi");
}
