use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::size_of;
use std::path::Path;


// ============================================================================
// FILE SYSTEM CONSTANTS AND STRUCTURES
// ============================================================================
// NOTE: These definitions are duplicated from the kernel codebase.
//
// WHY STANDALONE?
// In the C version, mkfs.c is compiled standalone with `gcc -o mkfs mkfs.c`
// and shares definitions via header files (fs.h, param.h, stat.h).
//
// Rust cannot use header files, so we duplicate the definitions here.
// This matches C's semantic model: mkfs is a host-side utility that runs
// on Linux/Mac (with std) to create fs.img BEFORE the kernel boots.
//
// The kernel (src/*) runs bare-metal x86 (no_std) and cannot share compiled
// code with host programs.
//
// MAINTENANCE: Keep synchronized with:
// - src/constants.rs (constants)
// - src/fs.rs (structs) - once created
// ============================================================================

// From fs.h (On-disk file system format)
const ROOTINO: u32 = 1;               // root i-number
const BSIZE: usize = 512;             // block size
const NDIRECT: usize = 12;            // number of direct block pointers
const NINDIRECT: usize = BSIZE / size_of::<u32>();  // indirect pointers
const MAXFILE: usize = NDIRECT + NINDIRECT;  // max file size in blocks
const DIRSIZ: usize = 14;             // directory name length

// From param.h (System parameters)
const FSSIZE: u32 = 1_000;           // size of file system in blocks
const NINODES: u32 = 200;             // number of inodes
const LOGSIZE: u32 = 30;              // max data blocks in on-disk log

// From stat.h (File types)
const T_DIR: u16 = 1;                 // Directory
const T_FILE: u16 = 2;                // File

// ============================================================================
// FILESYSTEM STRUCTURES (Duplicated from src/fs.rs)
// ============================================================================
// In C: These are defined in fs.h and included by both kernel and mkfs.c
// In Rust: Canonical definitions are in src/fs.rs (for kernel)
//          Duplicated here because mkfs is a standalone host binary
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Superblock {
	size: u32,
	nblocks: u32,
	ninodes: u32,
	nlog: u32,
	logstart: u32,
	inodestart: u32,
	bmapstart: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Dinode {
	typ: u16,
	major: u16,
	minor: u16,
	nlink: u16,
	size: u32,
	addrs: [u32; NDIRECT + 1],
}

impl Default for Dinode {
	fn default() -> Self {
		Self {
			typ: 0,
			major: 0,
			minor: 0,
			nlink: 0,
			size: 0,
			addrs: [0; NDIRECT + 1],
		}
	}
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Dirent {
	inum: u16,
	name: [u8; DIRSIZ],
}

impl Default for Dirent {
	fn default() -> Self {
		Self {
			inum: 0,
			name: [0; DIRSIZ],
		}
	}
}

struct Mkfs {
	fsfd: File,
	sb: Superblock,
	freeinode: u32,
	freeblock: u32,
}

fn as_bytes<T>(val: &T) -> &[u8] {
	unsafe { std::slice::from_raw_parts((val as *const T).cast::<u8>(), size_of::<T>()) }
}

fn from_bytes<T: Copy>(bytes: &[u8]) -> T {
	assert_eq!(bytes.len(), size_of::<T>());
	let mut out = std::mem::MaybeUninit::<T>::uninit();
	unsafe {
		std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.as_mut_ptr().cast::<u8>(), bytes.len());
		out.assume_init()
	}
}

fn name_to_dirent(name: &str, inum: u16) -> Dirent {
	let mut de = Dirent {
		inum: inum.to_le(),
		..Default::default()
	};
	let name_bytes = name.as_bytes();
	let copy_len = name_bytes.len().min(DIRSIZ);
	de.name[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
	de
}

impl Mkfs {
	fn new(fsfd: File, sb: Superblock, freeblock: u32) -> Self {
		Self {
			fsfd,
			sb,
			freeinode: 1,
			freeblock,
		}
	}

	fn wsect(&mut self, sec: u32, buf: &[u8; BSIZE]) {
		let off = (sec as u64) * (BSIZE as u64);
		self.fsfd
			.seek(SeekFrom::Start(off))
			.expect("lseek(write) failed");
		self.fsfd.write_all(buf).expect("write failed");
	}

	fn rsect(&mut self, sec: u32, buf: &mut [u8; BSIZE]) {
		let off = (sec as u64) * (BSIZE as u64);
		self.fsfd
			.seek(SeekFrom::Start(off))
			.expect("lseek(read) failed");
		self.fsfd.read_exact(buf).expect("read failed");
	}

	fn iblock(&self, inum: u32) -> u32 {
		(inum / ipb() as u32) + u32::from_le(self.sb.inodestart)
	}

	fn winode(&mut self, inum: u32, ip: &Dinode) {
		let mut buf = [0u8; BSIZE];
		let bn = self.iblock(inum);
		self.rsect(bn, &mut buf);

		let off = (inum as usize % ipb()) * size_of::<Dinode>();
		let src = as_bytes(ip);
		buf[off..off + size_of::<Dinode>()].copy_from_slice(src);
		self.wsect(bn, &buf);
	}

	fn rinode(&mut self, inum: u32) -> Dinode {
		let mut buf = [0u8; BSIZE];
		let bn = self.iblock(inum);
		self.rsect(bn, &mut buf);

		let off = (inum as usize % ipb()) * size_of::<Dinode>();
		from_bytes::<Dinode>(&buf[off..off + size_of::<Dinode>()])
	}

	fn ialloc(&mut self, typ: u16) -> u32 {
		let inum = self.freeinode;
		self.freeinode += 1;

		let din = Dinode {
			typ: typ.to_le(),
			nlink: 1u16.to_le(),
			size: 0u32.to_le(),
			..Default::default()
		};
		self.winode(inum, &din);
		inum
	}

	fn balloc(&mut self, used: u32) {
		assert!(used < (BSIZE * 8) as u32);
		println!("balloc: first {} blocks have been allocated", used);

		let mut buf = [0u8; BSIZE];
		for i in 0..used {
			let idx = (i / 8) as usize;
			let bit = (i % 8) as u8;
			buf[idx] |= 1u8 << bit;
		}

		let bmapstart = u32::from_le(self.sb.bmapstart);
		println!("balloc: write bitmap block at sector {}", bmapstart);
		self.wsect(bmapstart, &buf);
	}

	fn iappend(&mut self, inum: u32, mut p: &[u8]) {
		let mut din = self.rinode(inum);
		let mut off = u32::from_le(din.size);

		while !p.is_empty() {
			let fbn = (off as usize) / BSIZE;
			assert!(fbn < MAXFILE);

			let x: u32;
			if fbn < NDIRECT {
				if u32::from_le(din.addrs[fbn]) == 0 {
					din.addrs[fbn] = self.freeblock.to_le();
					self.freeblock += 1;
				}
				x = u32::from_le(din.addrs[fbn]);
			} else {
				if u32::from_le(din.addrs[NDIRECT]) == 0 {
					din.addrs[NDIRECT] = self.freeblock.to_le();
					self.freeblock += 1;
				}

				let mut indirect_sector = [0u8; BSIZE];
				let indirect_blockno = u32::from_le(din.addrs[NDIRECT]);
				self.rsect(indirect_blockno, &mut indirect_sector);

				let indirect_idx = fbn - NDIRECT;
				let byte_off = indirect_idx * size_of::<u32>();
				let mut indirect_entry = u32::from_le(from_bytes::<u32>(
					&indirect_sector[byte_off..byte_off + size_of::<u32>()],
				));

				if indirect_entry == 0 {
					indirect_entry = self.freeblock;
					self.freeblock += 1;
					let le = indirect_entry.to_le();
					indirect_sector[byte_off..byte_off + size_of::<u32>()].copy_from_slice(as_bytes(&le));
					self.wsect(indirect_blockno, &indirect_sector);
				}

				x = indirect_entry;
			}

			let mut buf = [0u8; BSIZE];
			self.rsect(x, &mut buf);

			let n1 = p
				.len()
				.min((fbn as u32 + 1) as usize * BSIZE - off as usize);
			let block_off = off as usize - (fbn * BSIZE);
			buf[block_off..block_off + n1].copy_from_slice(&p[..n1]);
			self.wsect(x, &buf);

			off += n1 as u32;
			p = &p[n1..];
		}

		din.size = off.to_le();
		self.winode(inum, &din);
	}
}

fn ipb() -> usize {
	BSIZE / size_of::<Dinode>()
}

fn main() {
	assert_eq!(size_of::<i32>(), 4, "Integers must be 4 bytes");
	assert_eq!(BSIZE % size_of::<Dinode>(), 0);
	assert_eq!(BSIZE % size_of::<Dirent>(), 0);

	let mut args = env::args().collect::<Vec<_>>();
	if args.len() < 2 {
		eprintln!("Usage: mkfs fs.img files...");
		std::process::exit(1);
	}

	let fs_img = args.remove(1);

	let nbitmap = FSSIZE / (BSIZE as u32 * 8) + 1;
	let ninodeblocks = NINODES / ipb() as u32 + 1;
	let nlog = LOGSIZE;
	let nmeta = 2 + nlog + ninodeblocks + nbitmap;
	let nblocks = FSSIZE - nmeta;

	let sb = Superblock {
		size: FSSIZE.to_le(),
		nblocks: nblocks.to_le(),
		ninodes: NINODES.to_le(),
		nlog: nlog.to_le(),
		logstart: 2u32.to_le(),
		inodestart: (2 + nlog).to_le(),
		bmapstart: (2 + nlog + ninodeblocks).to_le(),
	};

	println!(
		"nmeta {} (boot, super, log blocks {} inode blocks {}, bitmap blocks {}) blocks {} total {}",
		nmeta, nlog, ninodeblocks, nbitmap, nblocks, FSSIZE
	);

	let fsfd = OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(true)
		.open(&fs_img)
		.unwrap_or_else(|e| {
		eprintln!("{}: {}", fs_img, e);
		std::process::exit(1);
	});

	let mut mkfs = Mkfs::new(fsfd, sb, nmeta);

	let zeroes = [0u8; BSIZE];
	for i in 0..FSSIZE {
		mkfs.wsect(i, &zeroes);
	}

	let mut sb_buf = [0u8; BSIZE];
	sb_buf[..size_of::<Superblock>()].copy_from_slice(as_bytes(&mkfs.sb));
	mkfs.wsect(1, &sb_buf);

	let rootino = mkfs.ialloc(T_DIR);
	assert_eq!(rootino, ROOTINO);

	let dot = name_to_dirent(".", rootino as u16);
	mkfs.iappend(rootino, as_bytes(&dot));

	let dotdot = name_to_dirent("..", rootino as u16);
	mkfs.iappend(rootino, as_bytes(&dotdot));

	for path in &args[1..] {
		if path.contains('/') {
			eprintln!("{}: must be a basename (no /)", path);
			std::process::exit(1);
		}

		let mut host_file = File::open(path).unwrap_or_else(|e| {
			eprintln!("{}: {}", path, e);
			std::process::exit(1);
		});

		let file_name = if let Some(stripped) = path.strip_prefix('_') {
			stripped
		} else {
			Path::new(path)
				.file_name()
				.and_then(|s| s.to_str())
				.unwrap_or(path)
		};

		let inum = mkfs.ialloc(T_FILE);
		let de = name_to_dirent(file_name, inum as u16);
		mkfs.iappend(rootino, as_bytes(&de));

		let mut buf = [0u8; BSIZE];
		loop {
			let cc = host_file.read(&mut buf).expect("read input file failed");
			if cc == 0 {
				break;
			}
			mkfs.iappend(inum, &buf[..cc]);
		}
	}

	let mut din = mkfs.rinode(rootino);
	let mut off = u32::from_le(din.size);
	off = ((off / BSIZE as u32) + 1) * BSIZE as u32;
	din.size = off.to_le();
	mkfs.winode(rootino, &din);

	mkfs.balloc(mkfs.freeblock);
}
