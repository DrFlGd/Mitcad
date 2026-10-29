// SPDX-License-Identifier: MIT
//! Hostile sizes in design files (mitcad#928): a base body's B-rep data
//! that declares far more than its compressed data can hold, and a
//! library source's design in a project's store that is a zlib bomb, are
//! refused without allocating what they ask for. This crate's allocator
//! counts what the test's own thread allocates.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use serde_json::{Value, json};

use mitcad_model::Document;
use mitcad_model::features::Brep;
use mitcad_model::features::base::max_inflated;
use mitcad_model::file::{MemoryStore, Sha256, brep_path};
use mitcad_model::test_support::{block, def};
use mitcad_model::testing::MockKernel;

/// The system allocator, counting on a thread while it measures.
struct Counting;

/// What one thread allocated while measuring.
#[derive(Clone, Copy, Default)]
struct Allocated {
    /// The bytes allocated, and grown by reallocation, frees not
    /// subtracted: at least the thread's peak.
    total: usize,
    /// The largest single block.
    largest: usize,
}

std::thread_local! {
    /// Some while this thread measures. A const thread local without a
    /// destructor, so the allocator may use it.
    static ALLOCATED: Cell<Option<Allocated>> = const { Cell::new(None) };
}

fn count(grown: usize, size: usize) {
    let _ = ALLOCATED.try_with(|cell| {
        if let Some(mut allocated) = cell.get() {
            allocated.total = allocated.total.saturating_add(grown);
            allocated.largest = allocated.largest.max(size);
            cell.set(Some(allocated));
        }
    });
}

// SAFETY: every call goes to the system allocator unchanged; the counting
// only reads and writes a thread-local Cell and does not allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size(), layout.size());
        // SAFETY: the caller's contract is the system allocator's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size(), layout.size());
        // SAFETY: as in `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size.saturating_sub(layout.size()), new_size);
        // SAFETY: as in `alloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as in `alloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The result of `f` and what this thread allocated while it ran.
fn measured<T>(f: impl FnOnce() -> T) -> (T, Allocated) {
    ALLOCATED.set(Some(Allocated::default()));
    let result = f();
    let allocated = ALLOCATED.replace(None).unwrap_or_default();
    (result, allocated)
}

/// `n` bytes that zlib cannot compress.
fn noise(n: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 56) as u8
        })
        .collect()
}

/// Deflate's bits, least significant first.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    pending: u64,
    count: u32,
}

impl Bits {
    fn put(&mut self, value: u32, count: u32) {
        self.pending |= u64::from(value) << self.count;
        self.count += count;
        while self.count >= 8 {
            self.out.push(self.pending as u8);
            self.pending >>= 8;
            self.count -= 8;
        }
    }

    /// A Huffman code, its most significant bit first.
    fn code(&mut self, code: u32, length: u32) {
        let reversed = (0..length).fold(0, |r, i| r | (((code >> i) & 1) << (length - 1 - i)));
        self.put(reversed, length);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.out.push(self.pending as u8);
        }
        self.out
    }
}

/// Zlib data of `1 + 258 * repeats` zero bytes, written by hand so that a
/// test need not compress them: one block of deflate's fixed codes, a
/// literal zero and then copies of 258 bytes at distance 1, 13 bits each.
/// Returns the data and the size it decodes to.
fn zeros(repeats: usize) -> (Vec<u8>, u64) {
    let mut bits = Bits::default();
    bits.put(1, 1); // the last block
    bits.put(1, 2); // fixed codes
    bits.code(0x30, 8); // literal 0
    for _ in 0..repeats {
        bits.code(0xc5, 8); // length 258 (code 285)
        bits.code(0, 5); // distance 1 (code 0)
    }
    bits.code(0, 7); // end of block (code 256)
    let size = 1 + 258 * repeats as u64;
    let mut out = vec![0x78, 0x01];
    out.extend(bits.finish());
    // Adler-32 of `size` zero bytes.
    let adler = (((size % 65_521) as u32) << 16) | 1;
    out.extend(adler.to_be_bytes());
    (out, size)
}

/// The block design with a base feature whose one body has `data`, as
/// design file JSON.
fn with_base_body(data: Vec<u8>) -> Value {
    let mut b = block();
    let brep = serde_json::to_value(Brep::new(data)).unwrap();
    b.doc
        .add_feature(
            &def(json!({"type": "base", "bodies": [{"brep": brep}]})),
            None,
        )
        .unwrap();
    serde_json::from_str(&b.doc.to_json()).unwrap()
}

/// The first base body's B-rep in a design file's JSON.
fn base_brep(file: &mut Value) -> &mut Value {
    let base = file["features"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["type"] == "base")
        .expect("a base feature");
    &mut base["bodies"][0]["brep"]
}

#[test]
fn hand_written_zeros_are_valid_zlib() {
    let (stored, size) = zeros(10);
    assert_eq!(size, 2581);
    let zero = vec![0u8; 2581];
    let resolved = Brep::reference(Sha256::of(&zero), size)
        .resolve(&stored)
        .unwrap();
    assert_eq!(resolved.data(), Some(&zero[..]));
}

#[test]
fn a_base_body_declaring_gigabytes_is_refused_before_inflating() {
    let mut file = with_base_body(noise(1 << 20));
    let compressed = base_brep(&mut file)["data"].as_str().unwrap().len() * 3 / 4;
    assert!(compressed > 1_000_000, "{compressed}");
    let open = |file: &Value| {
        let text = file.to_string();
        measured(|| {
            Document::from_json(&text, MockKernel::default())
                .err()
                .map(|e| e.to_string())
        })
    };

    // The 4 GiB the ticket names: above the size limit.
    base_brep(&mut file)["size"] = json!(4_294_967_296u64);
    let (error, allocated) = open(&file);
    let error = error.expect("refused");
    assert!(
        error.contains("B-rep size 4294967296 is too large"),
        "{error}"
    );
    assert!(allocated.total < 100 << 20, "{}", allocated.total);

    // Below the size limit, but far more than 1 MB of zlib data holds.
    base_brep(&mut file)["size"] = json!(256u64 << 20);
    let (error, allocated) = open(&file);
    let error = error.expect("refused");
    assert!(
        error.contains("B-rep size 268435456 is too large for its"),
        "{error}"
    );
    assert!(allocated.total < 100 << 20, "{}", allocated.total);
}

#[test]
fn a_library_source_s_zlib_bomb_in_the_store_is_damaged() {
    // About 100 KB of zlib data that decodes to about 16 MB of zeros, more
    // than twice its cap. (Deflate packs at most about 1032 bytes in one,
    // so 100 KB cannot decode to 1 GiB; the cap is what matters.)
    let (stored, size) = zeros(63_000);
    assert!(stored.len() < 105_000, "{}", stored.len());
    let cap = max_inflated(stored.len());
    assert!(size > 2 * cap, "{size} {cap}");

    let design = Sha256::of(b"the library component's design");
    let store = MemoryStore::new();
    store.replace(design, stored);
    let mut file: Value = serde_json::from_str(&block().doc.to_json()).unwrap();
    file["version"] = json!(3);
    file["library_sources"] = json!([{
        "id": "S1",
        "library": {"library": "parts", "url": "https://example.org/parts.git",
                    "rev": "0123456789abcdef0123456789abcdef01234567",
                    "component": "bolt", "path": "bolt.mitcad"},
        "design": {"sha256": design.to_string()}
    }]);
    let text = file.to_string();

    let (document, allocated) =
        measured(|| Document::from_json_in(&text, &store, MockKernel::default()).unwrap());
    assert_eq!(
        document.load_warnings(),
        [format!(
            "library_sources[0] (S1): the file {} is damaged: it decodes to more than {cap} \
             bytes",
            brep_path(&design)
        )]
    );
    assert!(
        allocated.largest as u64 <= cap,
        "{} > {cap}",
        allocated.largest
    );
    assert!(document.state().library_sources.0[0].design.is_reference());
}
