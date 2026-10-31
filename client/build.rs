fn main() {
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
