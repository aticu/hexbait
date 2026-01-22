//! Models how the raw data is accessed in hexamine.

use std::{io, ops::Deref, path::Path, sync::Arc};

use memmap2::Mmap;
use positioned_io::{RandomAccessFile, ReadAt as _, Size as _};

use crate::{AbsoluteOffset, Len};

#[derive(Debug, Clone)]
pub struct Input(Arc<InputType>);

/// The input file to examine.
#[derive(Debug)]
enum InputType {
    /// The input is the given file.
    File {
        /// The open file handle.
        file: RandomAccessFile,
        /// The length of the file in bytes.
        len: u64,
    },
    /// The input is the given memory map.
    Memmap(Mmap),
    /// The input was read from stdin.
    Stdin(Box<[u8]>),
}

impl Input {
    /// Creates an input from the given path.
    pub fn from_path(path: impl AsRef<Path>) -> io::Result<Input> {
        /// Opens the path as an [`Mmap`].
        fn mmap_from_path(path: &Path) -> io::Result<Mmap> {
            let file = std::fs::File::open(path)?;

            let mut mmap_options = memmap2::MmapOptions::new();
            mmap_options.no_reserve_swap();

            Ok(unsafe {
                // SAFETY:
                // The file is only ever treated as bytes and most files opened with hexbait are
                // unlikely to be changed.
                // However this is no guarantee that this cannot mess up. I just think it's highly
                // unlikely that it would result in any vulnerability, but working with memmaps is
                // inherently unsafe.
                // Still the possible performance benefits are too great to ignore and it will not
                // cause any problems in 99% of the use cases where the opened files remain
                // unchanged.
                mmap_options.map(&file)?
            })
        }

        let path = path.as_ref();

        if let Ok(mmap) = mmap_from_path(path) {
            Ok(Input(Arc::new(InputType::Memmap(mmap))))
        } else {
            let file = positioned_io::RandomAccessFile::open(path).unwrap();
            let len = file
                .size()?
                .ok_or_else(|| io::Error::other("cannot get file size"))?;

            Ok(Input(Arc::new(InputType::File { file, len })))
        }
    }

    /// Creates an input from stdin.
    ///
    /// This should only be called once since it consumes stdin.
    pub fn from_stdin() -> io::Result<Input> {
        let mut buf = Vec::new();
        io::Read::read_to_end(&mut io::stdin(), &mut buf)?;

        Ok(Input(Arc::new(InputType::Stdin(buf.into()))))
    }

    /// The length of the data.
    pub fn len(&self) -> Len {
        match &*self.0 {
            InputType::File { len, .. } => Len::from(*len),
            InputType::Memmap(mmap) => Len::from(
                u64::try_from(mmap.len())
                    .expect("non `u64`-fitting length would not fit into memory"),
            ),
            InputType::Stdin(stdin) => Len::from(
                u64::try_from(stdin.len())
                    .expect("non `u64`-fitting length would not fit into memory"),
            ),
        }
    }

    /// Determines if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.len().is_zero()
    }

    /// Reads from the input at the given offset.
    ///
    /// If the requested start offset is beyond the end of the input an error is returned.
    /// If more input is requested than available, then the remaining input is returned.
    ///
    /// The `preallocated_buf` may be used in hot loops to avoid having to allocate a new buffer
    /// for every read.
    /// The buffer will be reused between iterations, if it is allocated outside the loop.
    pub fn read_at<'this_or_buf>(
        &'this_or_buf self,
        offset: AbsoluteOffset,
        len: Len,
        preallocated_buf: Option<&'this_or_buf mut Vec<u8>>,
    ) -> io::Result<ReadBytes<'this_or_buf>> {
        match &*self.0 {
            InputType::File {
                file,
                len: file_len,
                ..
            } => {
                if offset.as_u64() > *file_len {
                    return Err(io::Error::other("offset is beyond input"));
                }

                let len_left = *file_len - offset.as_u64();
                let output_size = std::cmp::min(len_left, len.as_u64())
                    .try_into()
                    .expect("we used min above, so this must fit into `buf`");

                Ok(if let Some(preallocated_buf) = preallocated_buf {
                    preallocated_buf.resize(output_size, 0);
                    file.read_exact_at(offset.as_u64(), &mut preallocated_buf[..output_size])?;

                    ReadBytes(ReadBytesInner::ByRef {
                        buf: &preallocated_buf[..output_size],
                    })
                } else if output_size <= READ_BYTES_INLINE_LEN {
                    let mut buf = [0u8; READ_BYTES_INLINE_LEN];
                    file.read_exact_at(offset.as_u64(), &mut buf)?;

                    ReadBytes(ReadBytesInner::Inline {
                        buf,
                        len: output_size as u8,
                    })
                } else {
                    let mut buf = vec![0u8; output_size].into_boxed_slice();
                    file.read_exact_at(offset.as_u64(), &mut buf)?;

                    ReadBytes(ReadBytesInner::Owned { buf })
                })
            }
            InputType::Memmap(mmap) => {
                let offset_usize: usize = offset
                    .as_u64()
                    .try_into()
                    .expect("offset does not fit into `usize`");

                if offset_usize > mmap.len() {
                    return Err(io::Error::other("offset is beyond input"));
                }

                let len_left = mmap.len() - offset_usize;
                let output_size = std::cmp::min(
                    len_left,
                    len.as_u64()
                        .try_into()
                        .expect("len does not fit into `usize`"),
                );

                Ok(ReadBytes(ReadBytesInner::ByRef {
                    buf: &mmap[offset_usize..offset_usize + output_size],
                }))
            }
            InputType::Stdin(stdin) => {
                let offset_usize: usize = offset
                    .as_u64()
                    .try_into()
                    .expect("offset does not fit into `usize`");

                if offset_usize > stdin.len() {
                    return Err(io::Error::other("offset is beyond input"));
                }

                let len_left = stdin.len() - offset_usize;
                let output_size = std::cmp::min(
                    len_left,
                    len.as_u64()
                        .try_into()
                        .expect("len does not fit into `usize`"),
                );

                Ok(ReadBytes(ReadBytesInner::ByRef {
                    buf: &stdin[offset_usize..offset_usize + output_size],
                }))
            }
        }
    }
}

/// Represents bytes that have been read from an input.
///
/// The bytes can be either owned, referenced or stored inline.
///
/// # Note on the pronunciation
///
/// Since the "read" in the type name refers to the past tense (as the bytes have already been read
/// when this type is obtained), it should be pronounced as such.
pub struct ReadBytes<'buf>(ReadBytesInner<'buf>);

/// The number of bytes that can be stored inline in a `ReadBytes`.
const READ_BYTES_INLINE_LEN: usize = 22;

/// The inner representation of the [`ReadBytes`] struct.
enum ReadBytesInner<'buf> {
    /// The bytes are stored as owned on the heap.
    Owned {
        /// The buffer on the heap.
        buf: Box<[u8]>,
    },
    /// The bytes are stored in a small inline slice.
    Inline {
        /// The buffer where the data is stored.
        buf: [u8; READ_BYTES_INLINE_LEN],
        /// The length of the buffer that is filled.
        len: u8,
    },
    /// The bytes are referenced from a different buffer.
    ByRef {
        /// The buffer that is referenced.
        buf: &'buf [u8],
    },
}

// Make sure that we don't grow larger than the already necessary 24 bytes.
const _: () = {
    assert!(std::mem::size_of::<ReadBytes<'static>>() == 24);
};

impl<'buf> Deref for ReadBytes<'buf> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            ReadBytesInner::Owned { buf } => buf,
            ReadBytesInner::Inline { buf, len } => &buf[..*len as usize],
            ReadBytesInner::ByRef { buf } => buf,
        }
    }
}

impl From<ReadBytes<'_>> for Vec<u8> {
    fn from(value: ReadBytes<'_>) -> Self {
        match value.0 {
            ReadBytesInner::Owned { buf } => buf.into(),
            ReadBytesInner::Inline { buf, len } => buf[..len as usize].into(),
            ReadBytesInner::ByRef { buf } => buf.into(),
        }
    }
}
