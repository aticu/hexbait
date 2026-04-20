//! Implements a tree in which bigram statistics are stored.

use std::{cmp, collections::BTreeMap};

use hexbait_common::{AbsoluteOffset, Len};

use crate::{statistics::handler::MIN_SAMPLE_SIZE, window::Window};

/// The branching factor of the tree.
///
/// This number determines how many child nodes must be joined to create the parent node.
const BRANCHING_FACTOR: u64 = 2;

/// The size of a leaf node in the tree.
pub const LEAF_NODE_SIZE: Len = MIN_SAMPLE_SIZE;

/// Stores bigram statistics for parts of an input in a tree.
///
/// # Invariants
///
/// - **Alignment**: Every node's offset is aligned to its tier's size, i.e.
///   `offset % tier.size() == 0`. This guarantees that a node's byte range is
///   always fully contained within any ancestor-tier-sized block, which the
///   insertion and promotion logic rely on.
///
/// - **Non-overlapping ranges**: No two nodes have overlapping byte ranges
///   `[offset, offset + tier.size())`. Insertion enforces this by removing any
///   descendants within the new node's range and any ancestor whose range
///   covers the new node's offset.
///
/// - **Sparse coverage**: The tree may have gaps - not every byte of the input
///   needs to be covered by a node. Use [`StatisticsTree::covers_window_exactly`]
///   to check whether a particular window is fully covered.
///
/// - **`memory_usage` consistency**: `memory_usage` always equals the sum of
///   `approximate_memory_usage()` across all stored nodes.
pub struct StatisticsTree<Statistics> {
    /// The nodes in the tree.
    nodes: BTreeMap<AbsoluteOffset, StatisticsTreeNode<Statistics>>,
    /// The current approximate memory usage of the tree.
    memory_usage: u64,
}

/// A node in the statistics tree.
pub struct StatisticsTreeNode<Statistics> {
    /// The current tier of the node.
    ///
    /// Lower is smaller, `0` is a node of size `LEAF_NODE_SIZE`, `1` is a node of size `BRANCHING_FACTOR * LEAF_NODE_SIZE` and so on.
    tier: Tier,
    /// The statistics of this node.
    statistics: Statistics,
}

/// The tier of a node in the statistics tree.
///
/// Larger means a larger area is covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tier(u8);

impl Tier {
    /// The tier of a leaf node.
    pub const LEAF_TIER: Tier = Tier(0);

    /// The maximum tier that should be computed at once.
    ///
    /// The reason for this to exist is to ensure that the background thread remains responsive.
    pub const MAX_DIRECT_TIER: Tier = Tier::fitting_tier(Len::from(8 * 1024 * 1024));

    /// Finds the smallest tier that has nodes smaller than the given length.
    pub const fn fitting_tier(len: Len) -> Tier {
        let mut tier = Tier::LEAF_TIER;
        while tier.next().size().as_u64() <= len.as_u64() {
            tier = tier.next();
        }

        tier
    }

    /// The next larger tier.
    pub const fn next(self) -> Tier {
        Tier(self.0 + 1)
    }

    /// The size of a node of the tier in bytes.
    pub const fn size(self) -> Len {
        Len::from(LEAF_NODE_SIZE.as_u64() * BRANCHING_FACTOR.pow(self.0 as u32))
    }
}

impl<Statistics: crate::statistics::Statistics> StatisticsTree<Statistics> {
    /// Creates a new statistics tree.
    pub fn new() -> StatisticsTree<Statistics> {
        StatisticsTree {
            nodes: BTreeMap::new(),
            memory_usage: 0,
        }
    }

    /// Inserts a node at the given tier into the tree.
    ///
    /// Removes any descendant nodes covered by the new node's range, and removes any ancestor node that overlaps (losing statistics for the ancestor's remaining range).
    #[track_caller]
    pub fn insert(&mut self, offset: AbsoluteOffset, tier: Tier, statistics: Statistics) {
        let size = tier.size();
        assert!(offset.is_aligned(size.as_u64()), "unaligned node insertion");
        let window = Window::from_start_len(offset, size);

        // Remove all nodes within our range (descendants or same-position).
        // Alignment guarantees any node starting in our range is fully contained.
        let start_offsets_to_remove = self
            .nodes
            .range(window.start()..window.end())
            .map(|(k, _)| *k)
            .collect::<Vec<_>>();
        for start_offset in start_offsets_to_remove {
            let node = self.nodes.remove(&start_offset).unwrap();
            self.memory_usage -= node.statistics.approximate_memory_usage();
        }

        // Remove an ancestor whose range covers our offset.
        if let Some((&ancestor_start, ancestor)) = self.nodes.range(..offset).next_back() {
            let ancestor_end = ancestor_start + ancestor.tier.size();
            if ancestor_end > offset {
                let node = self.nodes.remove(&ancestor_start).unwrap();
                self.memory_usage -= node.statistics.approximate_memory_usage();
            }
        }

        self.memory_usage += statistics.approximate_memory_usage();
        self.nodes
            .insert(offset, StatisticsTreeNode { tier, statistics });
    }

    /// Tries to promote the node at the given offset to the next tier.
    ///
    /// Returns the window of the newly promoted node, if it exists.
    fn try_promote(&mut self, offset: AbsoluteOffset) -> Option<Window> {
        let tier = match self.nodes.get(&offset) {
            Some(node) => node.tier,
            None => return None,
        };
        let parent_tier = tier.next();

        let size = tier.size();
        let parent_size = parent_tier.size();
        let parent_window =
            Window::from_start_len(offset.align_down(parent_size.as_u64()), parent_size);

        // first check if we can promote to avoid computing statistics partially that we need to drop because promotion is not possible
        let can_promote = parent_window.subwindows_of_size(size).all(|window| {
            self.nodes
                .get(&window.start())
                .map(|node| node.tier == tier)
                .unwrap_or(false)
        });
        if !can_promote {
            return None;
        }

        let mut parent_statistics = Statistics::empty();
        for window in parent_window.subwindows_of_size(size) {
            let node = self
                .nodes
                .remove(&window.start())
                .expect("we have checked that all child statistics ar present");
            self.memory_usage -= node.statistics.approximate_memory_usage();

            parent_statistics += &node.statistics;
        }

        self.memory_usage += parent_statistics.approximate_memory_usage();
        self.nodes.insert(
            parent_window.start(),
            StatisticsTreeNode {
                tier: parent_tier,
                statistics: parent_statistics,
            },
        );

        Some(parent_window)
    }

    /// Returns true if the statistics tree already covers the given window.
    pub fn covers_window_exactly(&self, window: Window) -> bool {
        let mut current = window.start();

        for (node_start, node) in self.nodes.range(window.start()..) {
            if *node_start != current {
                return false;
            }
            current += node.tier.size();
            if current == window.end() {
                return true;
            }
            if current > window.end() {
                return false;
            }
        }

        false
    }

    /// Aggregates statistics for the given window.
    ///
    /// Adds the stored statistics for the window into the given statistics.
    /// If more than `work_steps` steps would be performed, stop computation.
    /// Returns the offset where computation stopped.
    pub fn aggregate_for_window(
        &self,
        statistics: &mut Statistics,
        window: Window,
        work_steps: usize,
        min_tier: Tier,
    ) -> AbsoluteOffset {
        let mut steps_performed = 0;

        for (node_start, node) in self.nodes.range(window.start()..window.end()) {
            if node.tier < min_tier {
                // TODO: try promotion here to avoid useless recomputation
                continue;
            }
            let node_end = *node_start + node.tier.size();
            if node_end > window.end() {
                break;
            }

            *statistics += &node.statistics;
            steps_performed += 1;

            if steps_performed == work_steps {
                return node_end;
            }
        }

        window.end()
    }

    /// Prints debug statistics about the tree.
    #[allow(dead_code)]
    pub fn print_debug_statistics(&self) {
        let mut tier_stats = BTreeMap::<Tier, (usize, u64)>::new();
        for node in self.nodes.values() {
            let stats = tier_stats.entry(node.tier).or_default();
            stats.0 += 1;
            stats.1 += node.statistics.approximate_memory_usage();
        }
        eprint!(
            "mem: {}B",
            size_format::SizeFormatterBinary::new(self.memory_usage)
        );
        for (tier, stats) in tier_stats {
            eprint!(
                ", tier {} ({}B): {} nodes ({}B)",
                tier.0,
                size_format::SizeFormatterBinary::new(tier.size().as_u64()),
                stats.0,
                size_format::SizeFormatterBinary::new(stats.1)
            );
        }
        eprintln!()
    }

    /// Runs garbage collection by promoting nodes into their parents until
    /// memory usage drops to `memory_limit` or no more promotions are possible.
    ///
    /// `windows` must be ordered from outermost (largest) to innermost (smallest),
    /// with each window fully contained within the previous one. Nodes outside
    /// all windows are promoted first, preserving detail near the user's focus.
    ///
    /// Adjacent zones are kept within [`MAX_ZONE_TIER_GAP`] tiers of each other
    /// so that large jumps don't require full recomputation.
    ///
    /// Note: promotion requires all siblings at the same tier, so a node inside
    /// a high-priority zone may be consumed if a sibling outside that zone
    /// triggers promotion.
    pub fn garbage_collect(&mut self, memory_limit: u64, windows: &[Window]) {
        if self.memory_usage <= memory_limit {
            return;
        }

        debug_assert!(
            windows.windows(2).all(|pair| {
                pair[0].start() <= pair[1].start() && pair[0].end() >= pair[1].end()
            }),
            "windows must be nested from outermost to innermost"
        );

        loop {
            let mut candidates: Vec<(GcPriority, AbsoluteOffset)> = self
                .nodes
                .iter()
                .map(|(&offset, node)| {
                    let zone = node_zone(offset, windows);
                    let priority = GcPriority {
                        tier_adjusted: node.tier.0 as u64 + zone as u64 * MAX_ZONE_TIER_GAP,
                        distance_descending: cmp::Reverse(distance_from_higher_zone(
                            offset, zone, windows,
                        )),
                    };
                    (priority, offset)
                })
                .collect();

            candidates.sort();

            let memory_before = self.memory_usage;
            for (_, offset) in candidates {
                if self.memory_usage <= memory_limit {
                    return;
                }
                self.try_promote(offset);
            }

            // No promotion succeeded this pass - nothing more we can do.
            if self.memory_usage >= memory_before {
                return;
            }
        }
    }
}

/// The maximum tier difference allowed between adjacent priority zones
/// during garbage collection. Prevents outer zones from being coarsened
/// so aggressively that large jumps require full recomputation.
const MAX_ZONE_TIER_GAP: u64 = 2;

/// Sort key for garbage collection candidates.
///
/// Derives `Ord` so that `Vec::sort` gives the correct promotion order:
/// 1. Lower `tier_adjusted` first - interleaves zones so that no zone gets
///    more than `MAX_ZONE_TIER_GAP` tiers ahead of the next.
/// 2. Nodes farther from the next higher-priority zone boundary first.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct GcPriority {
    /// The tier adjusted by the `MAX_ZONE_GAP`.
    ///
    /// The formula is `tier + zone * MAX_ZONE_GAP`.
    tier_adjusted: u64,
    /// The distance from the next window.
    ///
    /// Sorted descending so lower distances are preferred.
    distance_descending: cmp::Reverse<u64>,
}
/// Returns the distance from the offset to the nearest edge of the next
/// higher-priority zone's boundary.
///
/// For the innermost zone (inside all windows), returns 0 since there is
/// no higher zone to measure distance from.
fn distance_from_higher_zone(offset: AbsoluteOffset, zone: usize, windows: &[Window]) -> u64 {
    if zone >= windows.len() {
        return 0;
    }
    let window = &windows[zone];
    if offset < window.start() {
        (window.start() - offset).as_u64()
    } else {
        (offset - window.end()).as_u64()
    }
}
/// Returns the priority zone of a node at the given offset.
///
/// Zone 0 is outside all windows (least important), zone `windows.len()`
/// is inside all windows (most important).
fn node_zone(offset: AbsoluteOffset, windows: &[Window]) -> usize {
    windows
        .iter()
        .position(|w| offset < w.start() || offset >= w.end())
        .unwrap_or(windows.len())
}
