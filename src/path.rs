use alloc::{
    borrow::ToOwned,
    str::Split,
    string::{String, ToString},
};
use core::{borrow::Borrow, ops::Deref, result::Result};

#[repr(transparent)]
pub struct Path {
    inner: str,
}

impl Path {
    pub fn new(path: &str) -> &Self {
        unsafe { &*(path as *const str as *const Path) }
    }
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.inner.split('/'),
        }
    }
    pub fn starts_with(&self, other: &Self) -> bool {
        self.inner.starts_with(&other.inner)
    }
    pub fn is_absolute(&self) -> bool {
        self.inner.starts_with("/")
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;
    fn to_owned(&self) -> Self::Owned {
        PathBuf::new(&self.inner)
    }
}

impl AsRef<Path> for &str {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        Path::new(&self.inner)
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a> IntoIterator for &'a Path {
    type Item = &'a str;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            inner: self.inner.split('/'),
        }
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub struct PathBuf {
    inner: String,
}

impl PathBuf {
    pub fn new(path: &str) -> Self {
        Self {
            inner: path.to_string(),
        }
    }
    pub fn strip_prefix(&self, prefix: &Self) -> Result<Self, ()> {
        if self.starts_with(prefix) {
            Ok(Self {
                inner: self.inner[prefix.inner.len()..].to_string(),
            })
        } else {
            Err(())
        }
    }
}

pub struct Iter<'a> {
    inner: Split<'a, char>,
}

impl Deref for PathBuf {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        Path::new(&self.inner)
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        Path::new(&self.inner)
    }
}
