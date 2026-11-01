fn main() {
    println!("cargo:rerun-if-changed=images/icon.ico");
    println!("cargo:rerun-if-changed=../target/release/cooplink.exe");
    println!("cargo:rerun-if-changed=../target/release/cooplink-viewer.exe");
    println!("cargo:rerun-if-changed=../target/release/cooplink-capture-service.exe");

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string());
    let out_path = std::path::Path::new(&out_dir);

    embed_file("cooplink.exe", "embedded_cooplink.bin", out_path);
    embed_file("cooplink-viewer.exe", "embedded_viewer.bin", out_path);
    embed_file("cooplink-capture-service.exe", "embedded_capture.bin", out_path);
    embed_file("cooplink-test-game.exe", "embedded_test_game.bin", out_path);

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

fn embed_file(src_name: &str, out_file: &str, out_path: &std::path::Path) {
    let dest = out_path.join(out_file);
    let candidates = [
        std::path::PathBuf::from(format!("../target/release/{}", src_name)),
        std::path::PathBuf::from(format!("../../target/release/{}", src_name)),
        std::path::PathBuf::from(format!("target/release/{}", src_name)),
        std::path::PathBuf::from(format!("bin/{}", src_name)),
        std::path::PathBuf::from(format!("test/{}", src_name)),
    ];

    let mut copied = false;
    for cand in &candidates {
        if cand.exists() {
            if let Ok(meta) = cand.metadata() {
                if meta.len() > 10_000 {
                    let _ = std::fs::copy(cand, &dest);
                    copied = true;
                    break;
                }
            }
        }
    }

    if !copied && !dest.exists() {
        let _ = std::fs::write(&dest, b"");
    }
}
