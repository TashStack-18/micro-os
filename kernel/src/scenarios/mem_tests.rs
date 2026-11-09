//! Allocator comparison: bump vs first-fit vs best-fit on the same
//! deterministic request stream, inside a private region carved from the heap.

use alloc::alloc::{alloc, dealloc};
use alloc::vec::Vec;
use core::alloc::Layout;

use microos_core::mem::{BumpAllocator, Fit, FreeListAllocator, RegionAllocator};
use microos_core::rng::XorShift64;

use super::{Options, Suite};
use crate::{memory, timer};

const REGION: usize = 256 * 1024;
const OPS: u32 = 4000;
const MAX_LIVE: usize = 200;

#[derive(Clone, Copy)]
enum Profile {
    /// 70% small (16-256 B), 25% medium (256-2048 B), 5% large (2-16 KiB).
    Mixed,
    /// Every request is 64 bytes.
    Uniform,
}

impl Profile {
    fn name(self) -> &'static str {
        match self {
            Profile::Mixed => "mixed",
            Profile::Uniform => "uniform64",
        }
    }

    fn size(self, rng: &mut XorShift64) -> usize {
        match self {
            Profile::Uniform => 64,
            Profile::Mixed => match rng.range(0, 100) {
                0..=69 => rng.range(16, 256) as usize,
                70..=94 => rng.range(256, 2048) as usize,
                _ => rng.range(2048, 16384) as usize,
            },
        }
    }
}

struct BenchResult {
    ok: u32,
    failed: u32,
    ns_per_op: u64,
    peak: usize,
    frag_avg_bp: u64,
    frag_max_bp: u64,
    rejected: u64,
    reclaimed_fully: bool,
}

fn bench(a: &mut dyn RegionAllocator, profile: Profile) -> BenchResult {
    let mut rng = XorShift64::new(0xC0FFEE);
    let mut live: Vec<(*mut u8, Layout)> = Vec::with_capacity(MAX_LIVE);
    let (mut ok, mut failed, mut ops, mut raw) = (0u32, 0u32, 0u64, 0u64);
    let (mut frag_sum, mut frag_max, mut samples) = (0u64, 0u64, 0u64);
    for op in 0..OPS {
        let allocate = live.is_empty() || (live.len() < MAX_LIVE && rng.chance(60));
        if allocate {
            let layout = Layout::from_size_align(profile.size(&mut rng), 8).unwrap();
            let t0 = timer::now_raw();
            let p = a.alloc(layout);
            raw += timer::now_raw() - t0;
            if p.is_null() {
                failed += 1;
            } else {
                ok += 1;
                live.push((p, layout));
            }
        } else {
            let (p, layout) = live.swap_remove(rng.range(0, live.len() as u64) as usize);
            let t0 = timer::now_raw();
            // SAFETY: `p` came from this allocator and is dropped from `live`.
            let r = unsafe { a.dealloc(p, layout) };
            raw += timer::now_raw() - t0;
            r.expect("benchmark free rejected");
        }
        ops += 1;
        if op % 100 == 99 {
            let f = a.stats().fragmentation_bp();
            frag_sum += f;
            frag_max = frag_max.max(f);
            samples += 1;
        }
    }
    let peak = a.stats().peak_bytes;
    for (p, layout) in live.drain(..) {
        // SAFETY: as above.
        unsafe { a.dealloc(p, layout) }.expect("final free rejected");
    }
    let end = a.stats();
    BenchResult {
        ok,
        failed,
        ns_per_op: timer::raw_to_ns(raw) / ops,
        peak,
        frag_avg_bp: frag_sum / samples.max(1),
        frag_max_bp: frag_max,
        rejected: end.rejected_frees,
        reclaimed_fully: end.bytes_in_use == 0 && end.largest_free + 16 >= end.capacity,
    }
}

pub fn allocator_comparison(s: &mut Suite, _o: &Options) {
    let region_layout = Layout::from_size_align(REGION, 4096).unwrap();
    // SAFETY: non-zero size; checked for null below.
    let region = unsafe { alloc(region_layout) };
    assert!(!region.is_null(), "could not reserve the benchmark region");
    let base = region as usize;

    for profile in [Profile::Mixed, Profile::Uniform] {
        let mut bump = BumpAllocator::empty();
        let mut first = FreeListAllocator::empty(Fit::First);
        let mut best = FreeListAllocator::empty(Fit::Best);
        let mut ok_counts = [0u32; 3];
        for i in 0..3 {
            // SAFETY: the region is ours until the dealloc below. Each
            // allocator is initialised over it right before its benchmark,
            // which frees everything it allocated before the next one starts.
            let a: &mut dyn RegionAllocator = unsafe {
                match i {
                    0 => {
                        bump.init(base, REGION);
                        &mut bump
                    }
                    1 => {
                        first.init(base, REGION);
                        &mut first
                    }
                    _ => {
                        best.init(base, REGION);
                        &mut best
                    }
                }
            };
            let res = bench(a, profile);
            ok_counts[i] = res.ok;
            kinfo!(
                "mem/{:<9} {:<9}: ok={:>4} failed={:>4} {:>5} ns/op peak={:>6} B frag avg={}.{:02}% max={}.{:02}%",
                profile.name(), a.name(), res.ok, res.failed, res.ns_per_op, res.peak,
                res.frag_avg_bp / 100, res.frag_avg_bp % 100, res.frag_max_bp / 100, res.frag_max_bp % 100
            );
            ktrace!(
                "MEM",
                "alloc={} profile={} ops={} ok={} failed={} ns_per_op={} peak={} frag_avg_bp={} frag_max_bp={} capacity={}",
                a.name(), profile.name(), OPS, res.ok, res.failed, res.ns_per_op, res.peak, res.frag_avg_bp, res.frag_max_bp, REGION
            );
            check!(
                s, "MEM.reclaims_everything", res.reclaimed_fully && res.rejected == 0,
                "{}/{}: all memory back in one free block after freeing everything", a.name(), profile.name()
            );
        }
        match profile {
            // Varied sizes: bump cannot reuse freed holes, so it runs out first.
            Profile::Mixed => check!(
                s, "MEM.freelist_beats_bump", ok_counts[1] > ok_counts[0] && ok_counts[2] > ok_counts[0],
                "mixed: successful allocations bump={} first-fit={} best-fit={}", ok_counts[0], ok_counts[1], ok_counts[2]
            ),
            // Fixed small size: the live set always fits, so strategy does not matter.
            Profile::Uniform => check!(
                s, "MEM.uniform_strategy_irrelevant", ok_counts[0] == ok_counts[1] && ok_counts[1] == ok_counts[2],
                "uniform64: bump={} first-fit={} best-fit={} (identical: no fragmentation possible)", ok_counts[0], ok_counts[1], ok_counts[2]
            ),
        }
    }

    // SAFETY: allocated above with the same layout; no allocator uses it now.
    unsafe { dealloc(region, region_layout) };
    let h = memory::stats();
    kinfo!(
        "kernel heap: in_use={} B peak={} B allocs={} frees={} free_blocks={} fragmentation={}.{:02}%",
        h.bytes_in_use, h.peak_bytes, h.allocs, h.frees, h.free_blocks, h.fragmentation_bp() / 100, h.fragmentation_bp() % 100
    );
}
