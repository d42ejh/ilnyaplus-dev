use cirrus_core::ecrs::*;
use std::{any, path::PathBuf};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    if args.len() != 2 {
        panic!("Usage: {} [target file path]", args.nth(0).unwrap());
    }
    let target_file_path = PathBuf::from(args.nth(1).unwrap());
    let out_path = std::env::current_dir().unwrap().join("out");
    std::fs::create_dir(&out_path)?;

    encode_file_to_blocks(&target_file_path, &out_path).await?;
    let outfile = std::env::current_dir().unwrap().join("dec_out");
    decode_blocks_to_file(&out_path, &outfile).await?;
    println!("All done");
    Ok(())
}
