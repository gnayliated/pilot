use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(&["src/remote.proto", "src/types.proto"], &["src/"])?;
    Ok(())
}
