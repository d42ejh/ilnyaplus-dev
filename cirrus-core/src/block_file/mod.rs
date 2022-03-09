use async_std::fs::{File, OpenOptions};
use async_std::prelude::*;
use openssl::rand::rand_bytes;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tracing::{event, instrument, span, Level};

// BlockFile supports
// read from nth block
// write to nth block
// todo write doc
pub struct BlockFile {
    file: File,
    path: PathBuf,
    max_span_size: u32,
    n: u32,
}

impl BlockFile {
    pub async fn new(path: &Path, max_span_size: u32) -> anyhow::Result<Self> {
        debug_assert_eq!(std::mem::size_of::<u64>(), 8);

        if path.file_name().is_none() {
            return Err(anyhow::Error::msg(format!(
                "Path: {:?} does not contain file name.",
                path
            )));
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .await?;

        //write max span size at the beginning of file
        file.write_all(&max_span_size.to_le_bytes()).await?;
        //write n
        file.write_all(&0_u32.to_le_bytes()).await?;
        //  file.sync_all().await.unwrap();
        Ok(BlockFile {
            file: file,
            path: path.to_owned(),
            max_span_size: max_span_size,
            n: 0,
        })
    }

    //todo return anyhow result Self
    pub async fn open(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Err(anyhow::Error::msg(format!("{:?} does not exist.", path)));
        }
        if !path.is_file() {
            return Err(anyhow::Error::msg(format!("{:?} is not file.", path)));
        }
        let mut file;
        let result = OpenOptions::new()
            .create(false)
            .create_new(false)
            .read(true)
            .write(true)
            .open(&path)
            .await;
        if result.is_err() {
            return Err(anyhow::Error::msg(format!(
                "Failed to open the block file {:?}.",
                path
            )));
        }
        file = result.unwrap();

        //read max span size from file
        debug_assert_eq!(std::mem::size_of::<u32>(), std::mem::size_of::<u8>() * 4);
        let mut array: [u8; 4] = [0; 4];
        file.read_exact(&mut array).await?;
        let max_span_size = u32::from_le_bytes(array);

        //read n from file
        file.read_exact(&mut array).await?;
        let n = u32::from_le_bytes(array);

        Ok(BlockFile {
            file: file,
            path: path.to_path_buf(),
            max_span_size: max_span_size,
            n: n,
        })
    }

    pub async fn write_nth_block(&mut self, nth: usize, buffer: &[u8]) -> anyhow::Result<()> {
        assert!((self.max_span_size as usize) >= buffer.len());

        //seek
        const U32_SIZE: usize = std::mem::size_of::<u32>();
        let seek_pos = nth * (U32_SIZE + self.max_span_size as usize) + (U32_SIZE * 2); //nth *(nth block size data + max span size) +(n + max spansize)
        let buffer_size: u32 = buffer.len() as u32;

        self.file.seek(SeekFrom::Start(seek_pos as u64)).await?;

        //write buffer length as byte
        self.file.write_all(&buffer_size.to_le_bytes()).await?;
        //write data
        self.file.write_all(buffer).await?;

        if buffer.len() < self.max_span_size as usize {
            //need to do zero padding
            let delta = self.max_span_size as usize - buffer.len();
            let pad = vec![0; delta];
            assert_eq!(pad.len(), delta);
            assert!(delta + buffer.len() == self.max_span_size as usize);
            self.file.write_all(&pad).await?;
        }

        //new n
        let next_n = std::cmp::max(self.n, nth.try_into().unwrap());
        if next_n == self.n {
            return Ok(());
        }
        //write new n to file
        self.file.seek(SeekFrom::Start(U32_SIZE as u64)).await?;
        self.file.write_all(&next_n.to_le_bytes()).await?;
        //update self.n
        self.n = next_n;
        Ok(())
    }

    pub async fn read_nth_block(&mut self, nth: usize) -> anyhow::Result<Vec<u8>> {
        const U32_SIZE: usize = std::mem::size_of::<u32>();

        //seek
        let meta = self.file.metadata().await?; //TODO: maybe need to sync()
        let seek_pos = nth * (U32_SIZE + self.max_span_size as usize) + (U32_SIZE * 2);
        self.file.seek(SeekFrom::Start(seek_pos as u64)).await?;

        //read span size from file
        let mut array: [u8; 4] = [0; 4];
        self.file.read_exact(&mut array).await?;

        let span_size: u32 = u32::from_le_bytes(array);
        assert!(span_size <= self.max_span_size);

        let mut buffer: Vec<u8> = vec![0; span_size as usize];
        debug_assert_eq!(buffer.len(), span_size as usize);

        //read span data
        self.file.read_exact(&mut buffer).await?;
        Ok(buffer)
    }

    pub fn n(&self) -> u32 {
        self.n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn block_file_test1() -> anyhow::Result<()> {
        /*
         tracing_subscriber::fmt()
             .with_thread_names(true)
             .with_max_level(Level::DEBUG)
             .init();
        */
        let path = std::env::current_dir().unwrap().join("testbf");
        let mut bf = BlockFile::new(&path, 30000).await?;
        let mut buf = Vec::new();

        //1
        buf = vec![1; 6];
        bf.write_nth_block(0, &buf).await?;
        //2
        buf = vec![2; 2];
        bf.write_nth_block(1, &buf).await?;

        //3
        buf = vec![245; 3];
        bf.write_nth_block(2, &buf).await?;
        //4
        buf = vec![255; 7];
        bf.write_nth_block(1024, &buf).await?;

        drop(bf);
        //reopen and verify
        let mut bf = BlockFile::open(&path).await?;

        //1
        buf = bf.read_nth_block(0).await?;
        assert_eq!(buf.len(), 6);
        assert_eq!(buf, vec![1; 6]);
        //2
        buf = bf.read_nth_block(1).await?;
        assert_eq!(buf.len(), 2);
        assert_eq!(buf, vec![2; 2]);

        //3
        buf = bf.read_nth_block(2).await?;
        assert_eq!(buf.len(), 3);
        assert_eq!(buf, vec![245; 3]);

        //4
        buf = bf.read_nth_block(1024).await?;
        assert_eq!(buf.len(), 7);
        assert_eq!(buf, vec![255; 7]);
        std::fs::remove_file(&path)?;
        Ok(())
    }

    #[tokio::test]
    async fn block_file_n_field_test() -> anyhow::Result<()> {
        let path = std::env::current_dir().unwrap().join("testbf2");
        let mut bf = BlockFile::new(&path, 1024).await?;

        for i in 0..256 {
            bf.write_nth_block(i, &i.to_le_bytes()).await?;
            assert_eq!(i, bf.n() as usize);
        }
        assert_eq!(bf.n(), 255);
        for i in 0..bf.n() {
            let buf = bf.read_nth_block(i as usize).await?;
            let x = usize::from_le_bytes(buf.try_into().unwrap());
            assert_eq!(x, i as usize);
        }

        std::fs::remove_file(&path)?;

        Ok(())
    }
}
