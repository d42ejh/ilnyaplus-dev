use crate::block_file::BlockFile;
use crate::ecrs::block::*;
use crate::ecrs::{CHK, SERIALIZED_CHK_BUFFER_SIZE};
use anyhow::Result;
use async_std::fs::{File, OpenOptions};
use async_std::prelude::*;
use openssl::hash::{hash, MessageDigest};
use openssl::rand::rand_bytes;
use openssl::symm::{decrypt, encrypt, Cipher};
use rkyv::ser::{serializers::AllocSerializer, Serializer};
use std::collections::VecDeque;
use std::io::SeekFrom;
use std::path::Path;
use tracing::{event, Level};

/// encode file to blocks
/// save the blocks to block files
/// return root(top) IBlock's CHK
pub async fn encode_file_to_blocks(file_path: &Path, block_file_dir: &Path) -> anyhow::Result<CHK> {
    assert!(file_path.is_file());
    assert!(file_path.file_name().is_some());
    assert!(block_file_dir.is_dir());
    assert!(block_file_dir.exists());

    let mut file = OpenOptions::new()
        .read(true)
        .open(&file_path)
        .await
        .unwrap();
    file.sync_all().await.unwrap();

    //paths for new block files
    let d_block_bf_path = block_file_dir.join("blocks.d");
    let d_block_chk_bf_path = block_file_dir.join("blocks.d.chk");
    let i_block_bf_path = block_file_dir.join("blocks.i");
    let i_block_chk_bf_path = block_file_dir.join("blocks.i.chk");

    //create new block files
    // for DBlock
    let mut d_block_bf =
        BlockFile::new(&d_block_bf_path, MAX_ENCRYPTED_DBLOCK_BUFFER_SIZE as u32).await?;

    // for DBlock CHK
    let mut d_block_chk_bf =
        BlockFile::new(&d_block_chk_bf_path, SERIALIZED_CHK_BUFFER_SIZE as u32).await?;

    // for IBlock
    let mut i_block_bf =
        BlockFile::new(&i_block_bf_path, MAX_ENCRYPTED_IBLOCK_BUFFER_SIZE as u32).await?;

    // for IBlock CHK
    let mut i_block_chk_bf =
        BlockFile::new(&i_block_chk_bf_path, SERIALIZED_CHK_BUFFER_SIZE as u32).await?;

    let metadata = file.metadata().await?;
    let file_length = metadata.len();

    //metadata for root IBlock
    let metadata = MetaData {
        file_name: file_path.file_name().unwrap().to_str().unwrap().to_owned(),
        file_size: file_length,
    };

    let d_block_count = calculate_d_block_count(file_length);
    let is_only_one_i_block = if calculate_total_i_block_count(d_block_count) == 1 {
        true
    } else {
        false
    };
    let mut buffer: Vec<u8> = vec![0; DBLOCK_SIZE_IN_BYTES as usize];

    //create DBlocks from file
    for i in 0..d_block_count {
        let seek_pos = i * DBLOCK_SIZE_IN_BYTES;
        file.seek(SeekFrom::Start(seek_pos)).await?;

        //on last round, resize buffer
        if i == d_block_count - 1 {
            buffer.resize((file_length - seek_pos) as usize, 0);
        }
        //read dblock data from file
        assert_ne!(buffer.len(), 0);
        if let Err(e) = file.read_exact(&mut buffer).await {
            return Err(anyhow::Error::msg(format!(
                "Failed to read {} bytes from file ({})\nfilesize: {}\ncurrent seek pos: {}",
                buffer.len(),
                e,
                file_length,
                seek_pos
            )));
        }
        //create new DBlock with buffer
        let d_block = DBlock::new(&buffer);
        //encrypt
        let (d_block_enc_key, d_block_enc_iv, d_block_enc_buffer, d_block_query_hash) =
            encrypt_d_block(&d_block);
        let d_block_chk = CHK::new(
            &d_block_enc_key,
            &d_block_enc_iv,
            &d_block_query_hash,
            BlockType::DBlock,
            i as u32,
        );

        //save DBlock and its CHK to BlockFIle
        d_block_bf
            .write_nth_block(i as usize, &d_block_enc_buffer)
            .await?;

        d_block_chk_bf
            .write_nth_block(i as usize, &d_block_chk.serialize())
            .await?;
        // event!(Level::DEBUG, "Done dblk {}", i);
    }
    event!(Level::DEBUG, "DBlock encode done");
    //Optimization is possible but aim clean and working implementation for now

    //do IBlock stuffs

    //create IBlocks with DBlock CHKs
    let total_i_block_count = calculate_total_i_block_count(d_block_count);
    let mut i_block_chks = VecDeque::new();
    let i_block_bf_index_base =
        total_i_block_count - calculate_nth_depth_i_block_count(d_block_count, 0);
    let mut current_i_block_count = 0; //done IBlock count
    let mut current_d_block_chk_count = 0;
    while current_d_block_chk_count != d_block_count {
        let d_block_chks_count = std::cmp::min(
            IBLOCK_CHK_CAPACITY,
            d_block_count - current_d_block_chk_count,
        );
        let mut d_block_chks = Vec::new();
        for i in 0..d_block_chks_count {
            //read DBlock CHK from block file
            let chk_bf_index = current_d_block_chk_count + i;
            let buffer = d_block_chk_bf.read_nth_block(chk_bf_index as usize).await?;
            let chk = CHK::from_bytes(&buffer);
            d_block_chks.push(chk);
        }
        event!(
            Level::DEBUG,
            "create {}th iblock",
            i_block_bf_index_base as usize + current_i_block_count
        );

        //create new IBlock with chks
        let i_block;
        if is_only_one_i_block {
            i_block = IBlock::new_root(&d_block_chks, &metadata);
        } else {
            i_block = IBlock::new(&d_block_chks);
        };
        let i_block_bf_index = i_block_bf_index_base as usize + current_i_block_count;
        //encrypt IBlock and save to block file
        let (key, iv, enc_buf, qh) = encrypt_i_block(&i_block);
        i_block_bf
            .write_nth_block(i_block_bf_index, &enc_buf)
            .await?;
        //create CHK
        let i_block_chk = CHK::new(&key, &iv, &qh, BlockType::IBlock, i_block_bf_index as u32);
        i_block_chks.push_back(i_block_chk);

        current_d_block_chk_count += d_block_chks_count;
        current_i_block_count += 1;
    }

    assert_eq!(
        current_i_block_count as u64,
        calculate_nth_depth_i_block_count(d_block_count, 0)
    );
    event!(
        Level::DEBUG,
        "Depth 0 IBlocks encode done with {} IBlocks",
        current_i_block_count
    );
    event!(Level::DEBUG, "chks len {}", i_block_chks.len());

    if is_only_one_i_block {
        assert_eq!(i_block_chks.len(), 1);
        //edge case
        // only 1 IBlock

        //save root IBlock's chk
        let chk = i_block_chks.pop_front().unwrap();
        i_block_chk_bf.write_nth_block(0, &chk.serialize()).await?;
        return Ok(chk);
    }

    // IBlock tree
    let mut current_depth = 1;
    while 1 < i_block_chks.len() && i_block_chks.len() <= IBLOCK_CHK_CAPACITY as usize {
        let current_depth_i_block_count =
            calculate_nth_depth_i_block_count(d_block_count, current_depth) as usize;
        //create t IBlocks
        let total_remain_i_block_count = total_i_block_count as usize - current_i_block_count;
        let available_chks = i_block_chks.len();

        event!(
            Level::DEBUG,
            "depth {} / current depth iblk count {} /total remain i block count {}
            / available chks {}",
            current_depth,
            current_depth_i_block_count,
            total_remain_i_block_count,
            available_chks
        );
        for i in 0..current_depth_i_block_count {
            let chks_for_this_i_block = std::cmp::min(available_chks, IBLOCK_CHK_CAPACITY as usize);
            let mut chks = Vec::new();
            //todo refactor
            for _ in 0..chks_for_this_i_block {
                assert!(!i_block_chks.is_empty());
                let chk = i_block_chks.pop_front().unwrap();
                chks.push(chk);
            }
            assert!(chks.len() > 0);

            let bf_index = total_remain_i_block_count - current_depth_i_block_count + i;
            let i_block;
            if bf_index == 0 {
                //root
                i_block = IBlock::new_root(&chks, &metadata);
            } else {
                i_block = IBlock::new(&chks);
            } //encrypt
            let (key, iv, enc_buf, qh) = encrypt_i_block(&i_block);
            //save IBlock to block file
            event!(Level::DEBUG, "Write {}th iblock", bf_index);
            i_block_bf.write_nth_block(bf_index, &enc_buf).await?;

            //IBlock CHK
            let chk = CHK::new(&key, &iv, &qh, BlockType::IBlock, bf_index as u32);
            i_block_chk_bf
                .write_nth_block(bf_index, &chk.serialize())
                .await?;
            if bf_index == 0 {
                return Ok(chk);
            }
        }
        current_depth += 1;
    }
    unreachable!(); //bless, smart compiler
}

pub async fn decode_blocks_to_file(
    block_file_dir: &Path,
    output_file_path: &Path,
) -> anyhow::Result<()> {
    //block file paths
    let d_block_bf_path = block_file_dir.join("blocks.d");
    let d_block_chk_bf_path = block_file_dir.join("blocks.d.chk");
    let i_block_bf_path = block_file_dir.join("blocks.i");
    let i_block_chk_bf_path = block_file_dir.join("blocks.i.chk");

    let mut d_block_bf = BlockFile::open(&d_block_bf_path).await?;
    let mut d_block_chk_bf = BlockFile::open(&d_block_chk_bf_path).await?; //TODO no need to use d block chks
    let mut i_block_bf = BlockFile::open(&i_block_bf_path).await?;
    let mut i_block_chk_bf = BlockFile::open(&i_block_chk_bf_path).await?;

    let mut output_file = File::create(&output_file_path).await?;

    let root_chk = CHK::from_bytes(&i_block_chk_bf.read_nth_block(0).await?);
    event!(Level::DEBUG, "root chk ok");
    let root_i_block = decrypt_i_block(
        &root_chk.key,
        &root_chk.iv,
        &i_block_bf.read_nth_block(0).await?,
    )?;

    event!(Level::DEBUG, "root i block ok {}", root_i_block.chks.len());
    let meta = root_i_block.metadata.unwrap();
    //let d_block_count = calculate_dblock_count(meta.file_size);
    let mut queue = VecDeque::new();
    root_i_block
        .chks
        .iter()
        .for_each(|chk| queue.push_back(chk.clone()));
    assert!(!queue.is_empty());
    while !queue.is_empty() {
        let chk: CHK = queue.pop_front().unwrap();
        event!(Level::DEBUG, "poped, Queue size {}", queue.len());
        if chk.block_type == BlockType::IBlock as u32 {
            event!(Level::DEBUG, "New IBlock",);
            let new_i_block = decrypt_i_block(
                &chk.key,
                &chk.iv,
                &i_block_bf.read_nth_block(chk.bf_index as usize).await?,
            )?;
            event!(
                Level::DEBUG,
                "Found IBlock with {} chks",
                new_i_block.chks.len()
            );

            new_i_block
                .chks
                .iter()
                .for_each(|chk| queue.push_back(chk.clone()));
        } else {
            event!(Level::DEBUG, "New DBlock",);
            //read DBlock
            let d_block = decrypt_d_block(
                &chk.key,
                &chk.iv,
                &d_block_bf.read_nth_block(chk.bf_index as usize).await?,
            )?;
            //seek
            let seek_pos = DBLOCK_SIZE_IN_BYTES * chk.bf_index as u64;
            output_file.seek(SeekFrom::Start(seek_pos)).await?;
            //write
            output_file.write_all(&d_block.data).await?;
        }
    }
    event!(Level::DEBUG, "Decode done");
    Ok(())
}

// Encrypt DBlock and return (key,iv,encrypted_buffer,queryhash)
#[must_use]
fn encrypt_d_block(dblock: &DBlock) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut serializer = AllocSerializer::<2048>::default(); //TODO: For now 2048
    serializer
        .serialize_value(dblock)
        .expect("Failed to serialize a message");
    let serialized_dblock_buffer = serializer.into_serializer().into_inner().to_vec();
    let plain_dblock_hash = hash(MessageDigest::sha3_512(), &serialized_dblock_buffer)
        .unwrap()
        .to_vec();
    let dblock_double_hash = hash(MessageDigest::sha3_256(), &plain_dblock_hash)
        .unwrap()
        .to_vec();

    //encrypt iblock
    let (iv, encrypted_dblock_buffer) =
        encrypt_chacha20_poly1305(&dblock_double_hash, &serialized_dblock_buffer);

    let dblock_query_hash = hash(MessageDigest::sha3_512(), &encrypted_dblock_buffer)
        .unwrap()
        .to_vec();
    assert_ne!(encrypted_dblock_buffer.len(), 0);
    assert!(encrypted_dblock_buffer.len() <= MAX_ENCRYPTED_DBLOCK_BUFFER_SIZE);

    (
        dblock_double_hash,
        iv,
        encrypted_dblock_buffer,
        dblock_query_hash,
    )
}

// Encrypt IBlock and return (key,iv,encrypted_buffer,queryhash)
#[must_use]
fn encrypt_i_block(iblock: &IBlock) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut serializer = AllocSerializer::<2048>::default(); //TODO: For now 2048
    serializer
        .serialize_value(iblock)
        .expect("Failed to serialize a message");
    let serialized_iblock_buffer = serializer.into_serializer().into_inner().to_vec();
    let plain_iblock_hash = hash(MessageDigest::sha3_512(), &serialized_iblock_buffer)
        .unwrap()
        .to_vec();
    let iblock_double_hash = hash(MessageDigest::sha3_256(), &plain_iblock_hash)
        .unwrap()
        .to_vec();

    //encrypt iblock
    let (iv, encrypted_iblock_buffer) =
        encrypt_chacha20_poly1305(&iblock_double_hash, &serialized_iblock_buffer);

    let iblock_query_hash = hash(MessageDigest::sha3_512(), &encrypted_iblock_buffer)
        .unwrap()
        .to_vec();
    assert!(encrypted_iblock_buffer.len() <= MAX_ENCRYPTED_IBLOCK_BUFFER_SIZE);
    (
        iblock_double_hash,
        iv,
        encrypted_iblock_buffer,
        iblock_query_hash,
    )
}
/*
pub fn encrypt_k_block(k_block: &KBlock) {
    //serialize
    let mut serializer = AllocSerializer::<2048>::default(); //TODO: For now 2048
    serializer
        .serialize_value(k_block)
        .expect("Failed to serialize a message");
    let serialized_k_block_buffer = serializer.into_serializer().into_inner().to_vec();
    let mut kw_hash = vec![0; public_key_bytes()];
    assert_eq!(kw_hash.len(), public_key_bytes());

    hash_xof(
        MessageDigest::sha3_512(),
        &k_block.keyword.as_bytes(),
        &mut kw_hash,
    )
    .unwrap();
    let (iv, enc_buf) = encrypt_chacha20_poly1305(&kw_hash, &serialized_k_block_buffer);
    //todo where to store the iv?
}
*/

#[must_use]
fn decrypt_d_block(key: &[u8], iv: &[u8], encrypted_buffer: &[u8]) -> Result<DBlock> {
    let dec_buf = decrypt_chacha20_poly1305(key, iv, encrypted_buffer);
    DBlock::from_bytes(&dec_buf)
}

#[must_use]
fn decrypt_i_block(key: &[u8], iv: &[u8], encrypted_buffer: &[u8]) -> Result<IBlock> {
    let dec_buf = decrypt_chacha20_poly1305(key, iv, encrypted_buffer);
    IBlock::from_bytes(&dec_buf)
}

//#[must_use]
//fn decrypt_k_block(encrypted_buffer: &[u8]) -> KBlock {}

#[must_use]
fn encrypt_chacha20_poly1305(key: &[u8], buffer: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let cipher = Cipher::chacha20_poly1305();
    assert!(cipher.iv_len().is_some());
    assert_eq!(cipher.key_len(), key.len());
    // generate fresh random IV
    let mut fresh_iv = vec![0; cipher.iv_len().unwrap()];
    rand_bytes(&mut fresh_iv).expect("Failed to generate IV");
    assert_eq!(fresh_iv.len(), cipher.iv_len().unwrap());

    //do encrypt
    let encrypted_buffer = encrypt(cipher, key, Some(&fresh_iv), buffer)
        .expect("Failed to encrypt with chacha20_poly1305");
    (fresh_iv, encrypted_buffer)
}

#[must_use]
fn decrypt_chacha20_poly1305(key: &[u8], iv: &[u8], buffer: &[u8]) -> Vec<u8> {
    let cipher = Cipher::chacha20_poly1305();
    assert_eq!(cipher.key_len(), key.len());
    assert!(cipher.iv_len().is_some());
    assert_eq!(cipher.iv_len().unwrap(), iv.len());
    decrypt(cipher, key, Some(iv), &buffer).unwrap()
}

//todo write test
fn calculate_d_block_count(file_length: u64) -> u64 {
    let is_multiple_of_d;
    if (file_length % DBLOCK_SIZE_IN_BYTES) == 0 {
        is_multiple_of_d = true;
    } else {
        is_multiple_of_d = false;
    }
    let mut dblock_count = file_length / DBLOCK_SIZE_IN_BYTES;
    if !is_multiple_of_d {
        dblock_count = dblock_count + 1;
    }
    dblock_count
}

fn calculate_total_i_block_count(d_block_count: u64) -> u64 {
    let mut total_count = 0;
    let mut cur_i_block_count = calculate_depth_zero_i_block_count(d_block_count);
    total_count += cur_i_block_count;
    while cur_i_block_count != 1 {
        if cur_i_block_count > IBLOCK_CHK_CAPACITY {
            let mut div = cur_i_block_count / IBLOCK_CHK_CAPACITY;
            let rem = cur_i_block_count % IBLOCK_CHK_CAPACITY;
            if rem != 0 {
                div += 1;
            }
            cur_i_block_count = div;
        } else {
            cur_i_block_count = 1;
        }

        total_count += cur_i_block_count;
    }
    total_count
}

fn calculate_nth_depth_i_block_count(d_block_count: u64, nth: u64) -> u64 {
    let mut cur_i_block_count = calculate_depth_zero_i_block_count(d_block_count);
    let mut loop_count = 0;
    if nth == 0 {
        return cur_i_block_count;
    }
    while loop_count < nth && cur_i_block_count != 1 {
        loop_count += 1;
        if cur_i_block_count > IBLOCK_CHK_CAPACITY {
            // apply 'f()'
            let mut div = cur_i_block_count / IBLOCK_CHK_CAPACITY;
            let rem = cur_i_block_count % IBLOCK_CHK_CAPACITY;
            if rem != 0 {
                div += 1;
            }
            cur_i_block_count = div;
        } else {
            cur_i_block_count = 1;
        }

        if loop_count == nth {
            return cur_i_block_count;
        }
    }
    0
}

fn calculate_depth_zero_i_block_count(d_block_count: u64) -> u64 {
    if d_block_count == 1 {
        return 1;
    }
    let rem = d_block_count % IBLOCK_CHK_CAPACITY;
    let div = d_block_count / IBLOCK_CHK_CAPACITY;
    if rem == 0 {
        return div;
    }
    div + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_block_calculation() {
        {
            //case 1
            let filesize = DBLOCK_SIZE_IN_BYTES * 10;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 10);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 1);

            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 1);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 0);
        }

        {
            //case2
            let filesize = DBLOCK_SIZE_IN_BYTES * 257;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 257);
            let i_count = calculate_depth_zero_i_block_count(d_count);
            assert_eq!(i_count, 2);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 3);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 2);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 1);
        }

        {
            //case3
            let filesize = DBLOCK_SIZE_IN_BYTES * 257 + 1;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 258);
            let i_count = calculate_depth_zero_i_block_count(d_count);
            assert_eq!(i_count, 2);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 3);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 2);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 1);
        }

        {
            //case4
            let filesize = 1;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 1);
            let i_count = calculate_depth_zero_i_block_count(d_count);
            assert_eq!(i_count, 1);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 1);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 1);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 0);
        }

        {
            let filesize = DBLOCK_SIZE_IN_BYTES * 1024 + 7;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 1025);
            let i_count = calculate_depth_zero_i_block_count(d_count);
            assert_eq!(i_count, 5);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 6);

            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 5);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 1);
        }

        {
            let filesize = (DBLOCK_SIZE_IN_BYTES * (DBLOCK_SIZE_IN_BYTES - 1)) * 2;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 65534);
            let i_count = calculate_depth_zero_i_block_count(d_count);
            assert_eq!(i_count, 256);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 257);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 256);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 1);
        }

        {
            let filesize = ((DBLOCK_SIZE_IN_BYTES * (DBLOCK_SIZE_IN_BYTES - 1)) * 2)
                + IBLOCK_CHK_CAPACITY * DBLOCK_SIZE_IN_BYTES;
            let d_count = calculate_d_block_count(filesize);
            assert_eq!(d_count, 65790);
            let i_count = calculate_depth_zero_i_block_count(d_count);
            assert_eq!(i_count, 257);
            let total_i = calculate_total_i_block_count(d_count);
            assert_eq!(total_i, 260);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 0), 257);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 1), 2);
            assert_eq!(calculate_nth_depth_i_block_count(d_count, 2), 1);
        }
    }

    async fn create_random_file(file_size: usize) -> std::path::PathBuf {
        assert_ne!(file_size, 0);
        let mut file_path = std::env::current_dir().unwrap();
        file_path.push("random_file");
        let mut file = File::create(&file_path).await.unwrap();
        let mut buf = Vec::new();
        buf.resize(file_size, 0);
        assert_eq!(buf.len(), file_size);

        rand_bytes(&mut buf).unwrap();
        file.write_all(&buf).await.unwrap();
        file.sync_all().await.unwrap();

        let meta = file.metadata().await.unwrap();
        assert_eq!(meta.len(), file_size as u64);
        drop(file);
        file_path
    }

    async fn compare_two_file(file1: &Path, file2: &Path) -> bool {
        assert!(file1.is_file());
        assert!(file2.is_file());
        let b1: Vec<u8> = async_std::fs::read(&file1).await.unwrap();
        let b2 = async_std::fs::read(&file2).await.unwrap();
        if b1 == b2 {
            return true;
        }
        false
    }

    #[tokio::test]
    async fn d_block_enc_dec_test() {
        let mut rb = vec![0; DBLOCK_SIZE_IN_BYTES as usize];
        rand_bytes(&mut rb).unwrap();
        let db = DBlock::new(&rb);
        let (key, iv, enc_buf, qh) = encrypt_d_block(&db);
        let ddb = decrypt_d_block(&key, &iv, &enc_buf);
        assert_eq!(db.data, ddb.data);
        assert_eq!(db.header, ddb.header);
    }

    #[tokio::test]
    async fn i_block_test() -> anyhow::Result<()> {
        let mut chks = Vec::new();
        for i in 0..IBLOCK_CHK_CAPACITY {
            let mut tmp = vec![0; 32];
            rand_bytes(&mut tmp).unwrap();
            let chk = CHK::new(&tmp, &tmp, &tmp, BlockType::IBlock, 0);
            chks.push(chk); //dummy
        }

        //test root IBlock
        let meta = MetaData {
            file_name: "test".to_string(),
            file_size: 99,
        };
        let ib = IBlock::new_root(&chks, &meta);
        let (key, iv, enc_buf, qh) = encrypt_i_block(&ib);

        //try decrypt
        let i_block = decrypt_i_block(&key, &iv, &enc_buf);
        assert_eq!(i_block.chks, ib.chks);
        assert_eq!(i_block.header, ib.header);
        assert!(i_block.metadata.is_some() && ib.metadata.is_some());
        assert_eq!(i_block.metadata.unwrap(), ib.metadata.unwrap());

        //try block file
        let temp_bf = std::env::current_dir().unwrap().join("tempibf");
        let mut bf = BlockFile::new(&temp_bf, MAX_ENCRYPTED_IBLOCK_BUFFER_SIZE as u32).await?;
        bf.write_nth_block(0, &enc_buf).await?;
        let b = bf.read_nth_block(0).await?;
        assert_eq!(b, enc_buf);
        std::fs::remove_file(temp_bf)?;

        //test normal IBlock
        let ib = IBlock::new(&chks);
        let (key, iv, enc_buf, qh) = encrypt_i_block(&ib);
        //try decrypt
        let i_block = decrypt_i_block(&key, &iv, &enc_buf);
        assert_eq!(i_block.chks, ib.chks);
        assert_eq!(i_block.header, ib.header);
        assert!(i_block.metadata.is_none() && ib.metadata.is_none());

        //try block file
        let temp_bf = std::env::current_dir().unwrap().join("tempibf");
        let mut bf = BlockFile::new(&temp_bf, MAX_ENCRYPTED_IBLOCK_BUFFER_SIZE as u32).await?;
        bf.write_nth_block(0, &enc_buf).await?;
        let b = bf.read_nth_block(0).await?;
        assert_eq!(b, enc_buf);
        std::fs::remove_file(temp_bf)?;
        Ok(())
    }

    // this test takes 1~2 minutes
    #[ignore] //ignore for now
    #[tokio::test]
    #[serial]
    async fn encode_file_to_blocks_test1() -> anyhow::Result<()> {
        //mid size file
        /*
           tracing_subscriber::fmt()
               .with_thread_names(true)
               .with_max_level(Level::DEBUG)
               .init();
        */
        let path =
            create_random_file((DBLOCK_SIZE_IN_BYTES * IBLOCK_CHK_CAPACITY * 30) as usize).await;
        let mut file = OpenOptions::new().read(true).open(&path).await?;
        let meta = file.metadata().await?;
        let file_length = meta.len();
        let d = calculate_d_block_count(file_length);
        let total_iblocks_count = calculate_total_i_block_count(d);

        println!("file {:?}", path);
        println!("file len {}", file_length);
        println!("d block count {}", d);
        println!("total i block count {}", total_iblocks_count);
        println!("encode to blocks");

        let temp_block_dir = std::env::current_dir().unwrap().join("temp");
        if !temp_block_dir.exists() {
            std::fs::create_dir(&temp_block_dir)?;
        }

        assert!(temp_block_dir.exists());
        assert!(temp_block_dir.is_dir());

        drop(file);
        encode_file_to_blocks(&path, &temp_block_dir).await?;
        //try decrypt
        let output_file_path = std::env::current_dir().unwrap().join("temp.dec");
        decode_blocks_to_file(&temp_block_dir, &output_file_path).await?;

        assert!(compare_two_file(&path, &output_file_path).await); //if this failed then it means the encryption, decryption scheme is fucked up

        //cleanup
        std::fs::remove_file(&path)?;
        std::fs::remove_dir_all(temp_block_dir)?;
        std::fs::remove_file(&output_file_path)?;
        Ok(())
    }

    //#[ignore]
    #[tokio::test]
    #[serial]
    async fn encode_file_to_blocks_test2() -> anyhow::Result<()> {
        //small file, 1 DBlock 1 IBlock
        /*
            tracing_subscriber::fmt()
                .with_thread_names(true)
                .with_max_level(Level::DEBUG)
                .init();
        */

        //create random file
        let path = create_random_file((DBLOCK_SIZE_IN_BYTES) as usize).await;
        let mut file = OpenOptions::new().read(true).open(&path).await?;
        let meta = file.metadata().await?;
        let file_length = meta.len();
        let d = calculate_d_block_count(file_length);
        let total_iblocks_count = calculate_total_i_block_count(d);

        println!("file {:?}", path);
        println!("file len {}", file_length);
        println!("d block count {}", d);
        println!("total i block count {}", total_iblocks_count);
        println!("encode to blocks");

        let temp_block_dir = std::env::current_dir().unwrap().join("temp");
        if !temp_block_dir.exists() {
            std::fs::create_dir(&temp_block_dir)?;
        }

        assert!(temp_block_dir.exists());
        assert!(temp_block_dir.is_dir());
        drop(file);

        encode_file_to_blocks(&path, &temp_block_dir).await?;
        //try decrypt
        let output_file_path = std::env::current_dir().unwrap().join("temp.dec");
        decode_blocks_to_file(&temp_block_dir, &output_file_path).await?;

        assert!(compare_two_file(&path, &output_file_path).await);

        //cleanup
        std::fs::remove_file(&path)?;
        std::fs::remove_dir_all(temp_block_dir)?;
        std::fs::remove_file(&output_file_path)?;
        Ok(())
    }
}
