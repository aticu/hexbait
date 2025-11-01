fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../format_descriptions/");

    let mut out = String::new();

    out.push_str("const BUILT_IN_DEFINITIONS_RAW: [(&'static str, &'static str); NUM_BUILTIN_DEFINITIONS] = [\n");

    let mut count = 0;
    for entry in std::fs::read_dir("../format_descriptions")? {
        let entry = entry?;
        let content = std::fs::read_to_string(entry.path())?;
        let name = entry
            .file_name()
            .into_string()
            .expect("built in format description with non utf8 name");

        out.push_str(&format!("    ({name:?}, {content:?}),\n"));
        count += 1;
    }

    out.push_str("];\n");
    out.push_str("\n");
    out.push_str(&format!(
        "const NUM_BUILTIN_DEFINITIONS: usize = {count};\n"
    ));

    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("built_in.gen.rs"), out).unwrap();

    Ok(())
}
