fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=styles.css");
    println!("cargo:rerun-if-changed=assets/iconify");
    // A committed subset of Lucide, so the build needs no network for its icons.
    topcoat::icon::iconify::BuildConfig::new()
        .icon_set_version("lucide", "1.2.137")
        .cache_dir("assets/iconify")
        .stage()
        .expect("stage the Lucide icon set");
    tablo_build::tailwind().expect("build the Tailwind stylesheet");
}
