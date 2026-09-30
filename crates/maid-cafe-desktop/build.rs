//! Windowsの実行ファイルにアイコンとバージョン情報を埋め込む(リソースコンパイラが無い環境では、埋め込みなしで続行する)。

fn main() {
    println!("cargo:rerun-if-changed=assets/maid-cafe-se.ico");
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/maid-cafe-se.ico");
        res.set("ProductName", "maid-cafe-se");
        res.set("FileDescription", "maid-cafe-se");
        res.set("CompanyName", "aon-co-jp");
        if let Err(e) = res.compile() {
            println!("cargo:warning=アイコンを埋め込めませんでした(続行します): {e}");
        }
    }
}
