//! Links the C++ runtime that intel_tex_2's prebuilt kernels need.

// intel_tex_2's prebuilt `ispc_texcomp_astc` archive is C++ and needs the C++ runtime, which
// its build script links only when it compiles the kernels itself (the `ispc` feature).
// see https://github.com/Traverse-Research/intel-tex-rs-2/blob/main/build.rs
// TODO: remove once intel_tex_2 links the C++ runtime for its prebuilt archives
fn main() {
  if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
    println!("cargo:rustc-link-lib=stdc++");
  }
}
