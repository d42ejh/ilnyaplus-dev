mod block;
mod chk;
mod encryption;

//exports
pub use block::*;
pub use chk::{CHK, SERIALIZED_CHK_BUFFER_SIZE};
pub use encryption::{decode_blocks_to_file, encode_file_to_blocks};

#[cfg(test)]
mod tests {

}
