fn main() {
    println!("cargo:rerun-if-changed=images/icon.ico");
    println!("cargo:rerun-if-changed=../target/release/cooplink.exe");

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string());
    let dest_bin = std::path::Path::new(&out_dir).join("embedded_cooplink.bin");

    // Recherche de cooplink.exe compilé pour l'embarquer dans le setup autonome
    let candidates = [
        std::path::PathBuf::from("../target/release/cooplink.exe"),
        std::path::PathBuf::from("../../target/release/cooplink.exe"),
        std::path::PathBuf::from("target/release/cooplink.exe"),
    ];

    let mut copied = false;
    for cand in &candidates {
        if cand.exists() {
            if let Ok(meta) = cand.metadata() {
                if meta.len() > 100_000 {
                    let _ = std::fs::copy(cand, &dest_bin);
                    copied = true;
                    break;
                }
            }
        }
    }

    if !copied && !dest_bin.exists() {
        let _ = std::fs::write(&dest_bin, b"");
    }

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("images/icon.ico");
        res.set("ProductName", "CoopLink");
        res.set("FileDescription", "CoopLink - Virtual LAN Gaming Engine");
        res.set("LegalCopyright", "Copyright (c) 2026 Loic HUON. All Rights Reserved.");
        if let Err(e) = res.compile() {
            eprintln!("Warning: could not compile Windows resource: {}", e);
        }
    }
}
