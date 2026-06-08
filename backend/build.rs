// Build script - locale dosyaları değiştiğinde yeniden derlemeyi tetikler ama sadece  code içindeki t! makrosu için gerekli tera template için gerekli değil
fn main() {
    // locales klasöründeki dosyalar değiştiğinde yeniden derle
    println!("cargo:rerun-if-changed=locales/");
    println!("cargo:rerun-if-changed=locales/tr.yml");
    println!("cargo:rerun-if-changed=locales/en.yml");
}
