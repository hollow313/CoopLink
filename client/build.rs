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

    // ATTENTION : `cfg!(windows)` teste la machine qui COMPILE. En compilation croisée
    // depuis Linux, le bloc était sauté et les exécutables sortaient sans icône, sans
    // version… et surtout sans manifeste, donc sans élévation : Wintun renvoyait alors
    // « Accès refusé (os error 5) » et le réseau local virtuel restait indisponible.
    println!("cargo:rerun-if-env-changed=COOPLINK_ADMIN_MANIFEST");
    let target_is_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    if target_is_windows {
        // `winres` échouait ici sans le dire (son manifeste était mal échappé et l'échec
        // n'était qu'un message perdu). On génère et compile la ressource nous-mêmes, et
        // toute erreur remonte en `cargo:warning`, donc visible.
        let admin = std::env::var("COOPLINK_ADMIN_MANIFEST").as_deref() == Ok("1");
        build_windows_resource(out_path, admin);
    }
}

/// Compile icône + VERSIONINFO (+ manifeste d'élévation) pour la cible Windows.
fn build_windows_resource(out_path: &std::path::Path, admin: bool) {
    use std::process::Command;

    let mut rc = String::from("#pragma code_page(65001)\n");
    match std::fs::canonicalize("images/icon.ico") {
        Ok(icon) => rc.push_str(&format!("1 ICON \"{}\"\n", icon.display())),
        Err(e) => println!("cargo:warning=icône absente ({}) : exécutable sans icône", e),
    }
    rc.push_str(
        r#"1 VERSIONINFO
FILEVERSION 1,0,0,0
PRODUCTVERSION 1,0,0,0
FILEOS 0x40004
FILETYPE 0x1
{
BLOCK "StringFileInfo"
{
BLOCK "040C04B0"
{
VALUE "CompanyName", "Loic HUON\0"
VALUE "FileDescription", "CoopLink - Virtual LAN Gaming Engine\0"
VALUE "FileVersion", "1.0.0\0"
VALUE "LegalCopyright", "Copyright (c) 2026 Loic HUON\0"
VALUE "ProductName", "CoopLink\0"
VALUE "ProductVersion", "1.0.0\0"
}
}
BLOCK "VarFileInfo"
{
VALUE "Translation", 0x40C, 1200
}
}
"#,
    );
    if admin {
        let manifest = out_path.join("app.manifest");
        if let Err(e) = std::fs::write(&manifest, ADMIN_MANIFEST) {
            println!("cargo:warning=manifeste non écrit ({}) : pas d'élévation", e);
            return;
        }
        rc.push_str(&format!("1 24 \"{}\"\n", manifest.display()));
    }

    let rc_path = out_path.join("app.rc");
    if let Err(e) = std::fs::write(&rc_path, rc) {
        println!("cargo:warning=script de ressources non écrit ({})", e);
        return;
    }
    let obj = out_path.join("app.res.o");
    let compiler = std::env::var("RC")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "x86_64-w64-mingw32-windres".to_string());
    let result = Command::new(&compiler)
        .arg("-i")
        .arg(&rc_path)
        .arg("-o")
        .arg(&obj)
        .arg("-O")
        .arg("coff")
        .output();
    match result {
        Ok(o) if o.status.success() => {
            println!("cargo:rustc-link-arg={}", obj.display());
        }
        Ok(o) => println!(
            "cargo:warning=ressources Windows non compilées ({}): {}",
            compiler,
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => println!(
            "cargo:warning=compilateur de ressources introuvable ({}): {}",
            compiler, e
        ),
    }
}


/// Manifeste du binaire principal : élévation demandée au lancement, sans quoi le pilote
/// Wintun (réseau local virtuel) et les règles de pare-feu échouent silencieusement.
const ADMIN_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">permonitorv2</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>"#;


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
