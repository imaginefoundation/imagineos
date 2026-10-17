use block::BlockDevice;
use std::cell::RefCell;
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::process::ExitCode;

#[path = "../kernel/src/drivers/block.rs"]
mod block;
#[path = "../kernel/src/fs/dfs.rs"]
mod dfs;
#[path = "../kernel/src/fs/gpt.rs"]
mod gpt;

mod ata {
    use crate::block::{BlockDevice, BlockError, SECTOR_SIZE};

    pub struct PrimaryMaster;

    impl BlockDevice for PrimaryMaster {
        fn sector_count(&self) -> u64 {
            0
        }

        fn read_sector(
            &self,
            _lba: u64,
            _buffer: &mut [u8; SECTOR_SIZE],
        ) -> Result<(), BlockError> {
            Err(BlockError::NotReady)
        }

        fn write_sector(&self, _lba: u64, _buffer: &[u8; SECTOR_SIZE]) -> Result<(), BlockError> {
            Err(BlockError::NotReady)
        }

        fn flush(&self) -> Result<(), BlockError> {
            Err(BlockError::NotReady)
        }
    }
}

struct ImageDisk(RefCell<std::fs::File>);

impl block::BlockDevice for ImageDisk {
    fn sector_count(&self) -> u64 {
        self.0
            .borrow()
            .metadata()
            .map_or(0, |metadata| metadata.len() / 512)
    }

    fn read_sector(
        &self,
        lba: u64,
        output: &mut [u8; block::SECTOR_SIZE],
    ) -> Result<(), block::BlockError> {
        let mut file = self.0.borrow_mut();
        file.seek(SeekFrom::Start(lba * block::SECTOR_SIZE as u64))
            .and_then(|_| file.read_exact(output))
            .map_err(|_| block::BlockError::DeviceError)
    }

    fn write_sector(
        &self,
        lba: u64,
        input: &[u8; block::SECTOR_SIZE],
    ) -> Result<(), block::BlockError> {
        let mut file = self.0.borrow_mut();
        file.seek(SeekFrom::Start(lba * block::SECTOR_SIZE as u64))
            .and_then(|_| file.write_all(input))
            .map_err(|_| block::BlockError::DeviceError)
    }

    fn flush(&self) -> Result<(), block::BlockError> {
        self.0
            .borrow_mut()
            .sync_all()
            .map_err(|_| block::BlockError::DeviceError)
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os().skip(1);
    let image = arguments
        .next()
        .ok_or_else(|| "usage: dfs-image DISK-IMAGE RAMFS-USTAR".to_owned())?;
    let ramfs = arguments
        .next()
        .ok_or_else(|| "usage: dfs-image DISK-IMAGE RAMFS-USTAR".to_owned())?;
    if arguments.next().is_some() {
        return Err("usage: dfs-image DISK-IMAGE RAMFS-USTAR".to_owned());
    }
    let device = ImageDisk(RefCell::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(image)
            .map_err(|error| format!("cannot open disk image: {error}"))?,
    ));
    let archive =
        std::fs::read(ramfs).map_err(|error| format!("cannot read RAMFS archive: {error}"))?;
    let table =
        gpt::Gpt::read_primary(&device).map_err(|error| format!("cannot read GPT: {error:?}"))?;
    let partition = table
        .find_partition(&device, &gpt::DFS_PARTITION_TYPE_GUID)
        .map_err(|error| format!("cannot find DFS partition: {error:?}"))?;
    let fs = dfs::Dfs::format(
        &device,
        partition.first_lba,
        partition.last_lba - partition.first_lba + 1,
    )
    .map_err(|error| format!("cannot format DFS: {error:?}"))?;
    fs.seed_from_ustar(&device, &archive)
        .map_err(|error| format!("cannot seed DFS: {error:?}"))?;
    device
        .flush()
        .map_err(|error| format!("cannot flush DFS: {error:?}"))?;
    println!(
        "Formatted and seeded DFS on GPT partition {}",
        partition.first_lba
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dfs-image: {error}");
            ExitCode::FAILURE
        }
    }
}
