use crate::{checked, path_bytes, syscall, Error, Result};

pub type Metadata = imagineos_abi::UserStat;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    create: bool,
    truncate: bool,
    append: bool,
}

impl OpenOptions {
    pub const fn new() -> Self {
        Self {
            read: false,
            write: false,
            create: false,
            truncate: false,
            append: false,
        }
    }

    pub const fn read(mut self, enabled: bool) -> Self {
        self.read = enabled;
        self
    }

    pub const fn write(mut self, enabled: bool) -> Self {
        self.write = enabled;
        self
    }

    pub const fn create(mut self, enabled: bool) -> Self {
        self.create = enabled;
        self
    }

    pub const fn truncate(mut self, enabled: bool) -> Self {
        self.truncate = enabled;
        self
    }

    pub const fn append(mut self, enabled: bool) -> Self {
        self.append = enabled;
        self
    }

    fn flags(self) -> u64 {
        (u64::from(self.read) * crate::abi::OPEN_READ)
            | (u64::from(self.write) * crate::abi::OPEN_WRITE)
            | (u64::from(self.create) * crate::abi::OPEN_CREATE)
            | (u64::from(self.truncate) * crate::abi::OPEN_TRUNCATE)
            | (u64::from(self.append) * crate::abi::OPEN_APPEND)
    }
}

pub struct File {
    fd: usize,
}

impl File {
    pub const fn descriptor(&self) -> usize {
        self.fd
    }

    pub fn read(&mut self, output: &mut [u8]) -> Result<usize> {
        read(self.fd, output)
    }

    pub fn write(&mut self, input: &[u8]) -> Result<usize> {
        write(self.fd, input)
    }

    pub fn close(self) -> Result<()> {
        close(self.fd)
    }
}

pub fn open(path: &str, options: OpenOptions) -> Result<File> {
    let path = path_bytes(path, crate::abi::MAX_MUTABLE_PATH)?;
    let flags = options.flags();
    let access = flags & (crate::abi::OPEN_READ | crate::abi::OPEN_WRITE);
    if access == 0
        || flags & (crate::abi::OPEN_TRUNCATE | crate::abi::OPEN_APPEND) != 0
            && flags & crate::abi::OPEN_WRITE == 0
        || flags & crate::abi::OPEN_TRUNCATE != 0 && flags & crate::abi::OPEN_APPEND != 0
    {
        return Err(Error::INVALID_ARGUMENT);
    }
    let fd = checked(syscall::invoke(
        crate::abi::SYS_OPEN,
        [path.as_ptr() as u64, path.len() as u64, flags, 0, 0, 0],
    ))?;
    Ok(File {
        fd: usize::try_from(fd).map_err(|_| Error::INVALID_ARGUMENT)?,
    })
}

pub fn read(fd: usize, output: &mut [u8]) -> Result<usize> {
    if output.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let count = checked(syscall::invoke(
        crate::abi::SYS_READ_FD,
        [
            fd as u64,
            output.as_mut_ptr() as u64,
            output.len() as u64,
            0,
            0,
            0,
        ],
    ))?;
    usize::try_from(count).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn write(fd: usize, input: &[u8]) -> Result<usize> {
    if input.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let count = checked(syscall::invoke(
        crate::abi::SYS_WRITE_FD,
        [
            fd as u64,
            input.as_ptr() as u64,
            input.len() as u64,
            0,
            0,
            0,
        ],
    ))?;
    usize::try_from(count).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn close(fd: usize) -> Result<()> {
    checked(syscall::invoke(
        crate::abi::SYS_CLOSE,
        [fd as u64, 0, 0, 0, 0, 0],
    ))
    .map(|_| ())
}

pub fn stdin() -> usize {
    crate::abi::FD_STDIN as usize
}

pub fn stdout() -> usize {
    crate::abi::FD_STDOUT as usize
}

pub fn stderr() -> usize {
    crate::abi::FD_STDERR as usize
}

pub fn disk_count() -> Result<usize> {
    let count = checked(syscall::invoke(crate::abi::SYS_DISK_COUNT, [0; 6]))?;
    usize::try_from(count).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn disk_sector_count(index: usize) -> Result<u64> {
    checked(syscall::invoke(
        crate::abi::SYS_DISK_SECTORS,
        [index as u64, 0, 0, 0, 0, 0],
    ))
}

pub fn install_to_disk(index: usize) -> Result<()> {
    checked(syscall::invoke(
        crate::abi::SYS_INSTALL_DISK,
        [index as u64, 0, 0, 0, 0, 0],
    ))
    .map(|_| ())
}

pub fn install_to_disk_with_config(index: usize, config: &crate::abi::InstallConfig) -> Result<()> {
    checked(syscall::invoke(
        crate::abi::SYS_INSTALL_DISK_CONFIG,
        [
            index as u64,
            (config as *const crate::abi::InstallConfig) as u64,
            0,
            0,
            0,
            0,
        ],
    ))
    .map(|_| ())
}

pub fn is_dir(path: &str) -> Result<bool> {
    let path = path_bytes(path, crate::abi::MAX_PATH_QUERY)?;
    Ok(checked(syscall::invoke(
        crate::abi::SYS_ISDIR,
        [path.as_ptr() as u64, path.len() as u64, 0, 0, 0, 0],
    ))? != 0)
}

pub fn is_file(path: &str) -> Result<bool> {
    let path = path_bytes(path, crate::abi::MAX_PATH_QUERY)?;
    Ok(checked(syscall::invoke(
        crate::abi::SYS_ISFILE,
        [path.as_ptr() as u64, path.len() as u64, 0, 0, 0, 0],
    ))? != 0)
}

pub fn read_file(path: &str, output: &mut [u8]) -> Result<usize> {
    let path = path_bytes(path, crate::abi::MAX_PATH_QUERY)?;
    if output.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let result = checked(syscall::invoke(
        crate::abi::SYS_READ_FILE,
        [
            path.as_ptr() as u64,
            path.len() as u64,
            output.as_mut_ptr() as u64,
            output.len() as u64,
            0,
            0,
        ],
    ))?;
    usize::try_from(result).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn list_directory(path: &str, output: &mut [u8]) -> Result<usize> {
    list_directory_with_hidden(path, output, false)
}

pub fn list_directory_with_hidden(
    path: &str,
    output: &mut [u8],
    include_hidden: bool,
) -> Result<usize> {
    let path = path_bytes(path, crate::abi::MAX_PATH_QUERY)?;
    if output.len() > crate::abi::MAX_READ_BUFFER {
        return Err(Error::INVALID_ARGUMENT);
    }
    let result = checked(syscall::invoke(
        crate::abi::SYS_READDIR,
        [
            path.as_ptr() as u64,
            path.len() as u64,
            output.as_mut_ptr() as u64,
            output.len() as u64,
            0,
            include_hidden as u64,
        ],
    ))?;
    usize::try_from(result).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn metadata(path: &str) -> Result<Metadata> {
    let path = path_bytes(path, crate::abi::MAX_PATH_QUERY)?;
    let mut metadata = Metadata::default();
    checked(syscall::invoke(
        crate::abi::SYS_STAT,
        [
            path.as_ptr() as u64,
            path.len() as u64,
            (&mut metadata as *mut Metadata) as u64,
            0,
            0,
            0,
        ],
    ))?;
    Ok(metadata)
}

pub fn write_file(path: &str, contents: &[u8]) -> Result<usize> {
    let path = path_bytes(path, crate::abi::MAX_MUTABLE_PATH)?;
    if contents.len() > crate::abi::MAX_WRITE_FILE_SIZE {
        return Err(Error::NO_SPACE);
    }
    let result = checked(syscall::invoke(
        crate::abi::SYS_WRITE_FILE,
        [
            path.as_ptr() as u64,
            path.len() as u64,
            contents.as_ptr() as u64,
            contents.len() as u64,
            0,
            0,
        ],
    ))?;
    usize::try_from(result).map_err(|_| Error::INVALID_ARGUMENT)
}

pub fn mkdir(path: &str, parents: bool) -> Result<()> {
    let path = path_bytes(path, crate::abi::MAX_MUTABLE_PATH)?;
    checked(syscall::invoke(
        crate::abi::SYS_MKDIR,
        [
            path.as_ptr() as u64,
            path.len() as u64,
            parents as u64,
            0,
            0,
            0,
        ],
    ))
    .map(|_| ())
}

pub fn touch(path: &str) -> Result<()> {
    let path = path_bytes(path, crate::abi::MAX_MUTABLE_PATH)?;
    checked(syscall::invoke(
        crate::abi::SYS_TOUCH,
        [path.as_ptr() as u64, path.len() as u64, 0, 0, 0, 0],
    ))
    .map(|_| ())
}

pub fn remove(path: &str, recursive: bool) -> Result<()> {
    let path = path_bytes(path, crate::abi::MAX_MUTABLE_PATH)?;
    checked(syscall::invoke(
        crate::abi::SYS_REMOVE,
        [
            path.as_ptr() as u64,
            path.len() as u64,
            recursive as u64,
            0,
            0,
            0,
        ],
    ))
    .map(|_| ())
}

pub struct DirectoryEntries<'a> {
    remaining: &'a [u8],
}

impl<'a> DirectoryEntries<'a> {
    pub const fn new(buffer: &'a [u8]) -> Self {
        Self { remaining: buffer }
    }
}

impl<'a> Iterator for DirectoryEntries<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        while !self.remaining.is_empty() {
            let end = self
                .remaining
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(self.remaining.len());
            let (entry, rest) = self.remaining.split_at(end);
            self.remaining = rest.strip_prefix(b"\n").unwrap_or(rest);
            if !entry.is_empty() {
                return Some(entry);
            }
        }
        None
    }
}
