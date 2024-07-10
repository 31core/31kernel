use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::result::Result;

pub type Path = [String];

pub static mut ROOT_VFS: Option<VirtualFileSystem> = None;

#[derive(Default)]
pub struct VirtualFileSystem {
    pub mount_points: Vec<Vec<String>>,
    pub mounted_fs: Vec<Box<dyn FileSystem>>,
}

impl VirtualFileSystem {
    pub fn open(&mut self, path: &Path) -> Result<File, ()> {
        let mut fs = None;
        let mut found_mountpoint_depth = 0;
        'main: for (p, point) in self.mount_points.iter().enumerate() {
            for (i, entry) in point.iter().enumerate() {
                if i > path.len() && path[i] != *entry {
                    continue 'main;
                }
            }

            if point.len() > found_mountpoint_depth {
                fs = Some(&mut self.mounted_fs[p]);
                found_mountpoint_depth = point.len();
            }
        }

        if let Some(fs) = fs {
            fs.open(&path[found_mountpoint_depth..])
        } else {
            Err(())
        }
    }
    pub fn mount(&mut self, fs: Box<dyn FileSystem>, mount_point: &Path) {
        self.mounted_fs.push(fs);
        self.mount_points.push(mount_point.to_vec());
    }
    pub fn umount(&mut self, mount_point: &Path) {
        'main: for (i, mpoint) in self.mount_points.iter().enumerate() {
            for (j, entry) in mount_point.iter().enumerate() {
                if j > mpoint.len() && *entry != mpoint[j] {
                    continue 'main;
                }
            }
            self.mount_points.remove(i);
            break;
        }
    }
}

pub enum FileType {
    RegularFile,
    Directory,
    CharDev,
    BlockDev,
}

pub struct File {
    pub fd: u64,
    pub r#type: FileType,
}

pub trait FileSystem {
    fn create(&mut self, path: &Path) -> Result<File, ()>;
    fn open(&mut self, path: &Path) -> Result<File, ()>;
    fn write(&mut self, fd: &File, buf: &[u8]) -> Result<u64, ()>;
    fn read(&mut self, fd: &File, buf: &mut [u8], offset: u64) -> Result<u64, ()>;
    fn remove(&mut self, path: &Path) -> Result<(), ()>;
    fn rename(&mut self, src: &Path, dst: &Path) -> Result<(), ()>;
    fn close(&mut self, fd: &File) -> Result<(), ()>;
    fn list_dir(&mut self) -> Result<Vec<String>, ()>;
}
