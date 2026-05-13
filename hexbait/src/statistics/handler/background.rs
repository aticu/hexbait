//! Defines a thread to compute statistics in the background.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        mpsc::{self, RecvError, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use hexbait_common::{Input, Len};

use crate::{
    statistics::{
        BigramStatistics, StatisticsMetrics,
        handler::{
            CalculationResult, Request, WindowMetrics,
            background::{statistics_tree::StatisticsTree, work_phase::WorkPhase},
            compute_bin_size_and_align_window,
        },
    },
    window::Window,
};

mod statistics_tree;
mod work_phase;

/// The target frames per second at which updates should be sent to the frontend.
const TARGET_FPS: u64 = 60;

/// Contains the result of starting a background statistics engine.
pub struct BackgroundStatisticsEngineStartResult {
    /// The channel over which requests can be sent to the backend.
    pub request_channel: mpsc::Sender<Request>,
    /// The result view shared with the backend.
    pub result: Arc<ArcSwap<CalculationResult>>,
}

/// The state stored by the backend to efficiently compute requests.
pub struct BackgroundStatisticsEngine {
    /// The channel over which the frontend sends requests.
    request_channel: mpsc::Receiver<Request>,
    /// The result view shared with the frontend.
    result: Arc<ArcSwap<CalculationResult>>,
    /// The state of computed data.
    computation_state: ComputationState,
    /// The work phase the background thread is in.
    work_phase: WorkPhase,
}

impl BackgroundStatisticsEngine {
    /// Starts a new background statistics engine.
    pub fn start(input: Input) -> BackgroundStatisticsEngineStartResult {
        let (send, recv) = mpsc::channel();
        let result = Arc::new(ArcSwap::from_pointee(CalculationResult {
            metrics: Vec::new(),
            statistics: BigramStatistics::empty(),
            selected_window: Window::ZERO,
        }));
        let frontend_result = Arc::clone(&result);

        thread::spawn(move || {
            let background_state = BackgroundStatisticsEngine {
                request_channel: recv,
                result,
                computation_state: ComputationState::new(input),
                work_phase: WorkPhase::Idle,
            };

            background_state.run();
        });

        BackgroundStatisticsEngineStartResult {
            request_channel: send,
            result: frontend_result,
        }
    }

    /// Publishes work performed by the backend.
    fn publish_work(&mut self) {
        let metrics = self
            .computation_state
            .derived_values
            .iter()
            .map(|(&window, &metrics)| WindowMetrics { window, metrics })
            .collect::<Vec<_>>();
        let (_, selected_window) = self
            .computation_state
            .innermost_bin_size_and_aligned_window();

        let result = CalculationResult {
            metrics,
            statistics: self.computation_state.current_window_statistics.clone(),
            selected_window,
        };

        self.result.store(Arc::new(result));
    }

    /// Resets the state for a new request.
    fn reset_for_request(&mut self, request: Request) {
        self.computation_state.reset_for_request(request);
        self.work_phase = WorkPhase::from_beginning(&mut self.computation_state);
    }

    /// Performs garbage collection to keep the memory usage as low as possible.
    fn do_garbage_collection(&mut self) {
        if let Some(request) = &self.computation_state.latest_request {
            self.computation_state
                .statistics_tree
                .garbage_collect(800 * 1024 * 1024, &request.windows);
        }
        // TODO: choose a smarter limit
        // TODO: garbage collect scalars as well
    }

    /// Determines if there is still work left.
    fn has_more_work(&self) -> bool {
        !matches!(self.work_phase, WorkPhase::Idle)
    }

    /// Processes new requests.
    ///
    /// Returns `false` if the thread should terminate.
    fn process_new_requests(&mut self) -> bool {
        let mut request = if self.has_more_work() {
            match self.request_channel.try_recv() {
                Ok(request) => request,
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        } else {
            // we will possibly block now, so let's ensure that we publish one last time and garbage collect
            if self.computation_state.latest_request.is_some() {
                self.publish_work();
                self.do_garbage_collection();
            }

            match self.request_channel.recv() {
                Ok(request) => request,
                Err(RecvError) => return false,
            }
        };

        // receive all new requests at once to avoid starting one and then immediately cancelling it during fast scrolling
        loop {
            match self.request_channel.try_recv() {
                Ok(new_request) => request = new_request,
                Err(TryRecvError::Empty) => {
                    break;
                }
                Err(TryRecvError::Disconnected) => return false,
            };
        }

        self.reset_for_request(request);

        true
    }

    /// Runs the background computations.
    fn run(mut self) {
        loop {
            if !self.process_new_requests() {
                break;
            }
            self.publish_work();
            self.do_garbage_collection();

            self.work_phase.advance(&mut self.computation_state);
        }
    }
}

/// The state shared by computations done by the backend.
struct ComputationState {
    /// The input that computations are based on.
    input: Input,
    /// The latest request for what should be computed.
    latest_request: Option<Request>,
    /// All values derived from statistics.
    derived_values: BTreeMap<Window, StatisticsMetrics>,
    /// The tree of statistics information.
    statistics_tree: StatisticsTree<BigramStatistics>,
    /// The computed statistics of the current window.
    current_window_statistics: BigramStatistics,
    /// The last time when an update was sent to the frontend.
    last_yield: Instant,
}

impl ComputationState {
    /// Creates a new `ComputationState`.
    fn new(input: Input) -> ComputationState {
        ComputationState {
            input,
            latest_request: None,
            derived_values: BTreeMap::new(),
            statistics_tree: StatisticsTree::new(),
            current_window_statistics: BigramStatistics::empty(),
            last_yield: Instant::now(),
        }
    }

    /// Resets the computation state for a new request.
    fn reset_for_request(&mut self, request: Request) {
        self.current_window_statistics = BigramStatistics::empty();
        self.last_yield = Instant::now();
        self.latest_request = Some(request);
    }

    /// Yields by returning `None` if a yield is necessary.
    ///
    /// This allows to easily bubble yields using `?`.
    fn maybe_yield(&mut self) -> Option<()> {
        const UPDATE_DURATION: Duration = Duration::from_nanos(1_000_000_000 / TARGET_FPS);

        if self.last_yield.elapsed() > UPDATE_DURATION {
            self.last_yield = Instant::now();
            None
        } else {
            Some(())
        }
    }

    /// Returns the bin size and the aligned window for the given `window_index`.
    ///
    /// # Panics
    ///
    /// This fuction may panic if
    /// - the window index is greater than or equal to the number of windows.
    /// - there is no current request.
    fn bin_size_and_aligned_window(&self, window_index: usize) -> (Len, Window) {
        let request = self.latest_request.as_ref().unwrap();
        compute_bin_size_and_align_window(request.windows[window_index], request.bins_per_window)
    }

    /// Returns the bin size and the aligned window for the innermost window.
    ///
    /// # Panics
    ///
    /// This fuction may panic if
    /// - there is no current request.
    fn innermost_bin_size_and_aligned_window(&self) -> (Len, Window) {
        let request = self.latest_request.as_ref().unwrap();
        compute_bin_size_and_align_window(
            *request.windows.last().unwrap(),
            request.bins_in_innermost_window,
        )
    }

    /// Returns the index of the last window in the current request.
    fn last_window_index(&self) -> usize {
        self.latest_request.as_ref().unwrap().windows.len() - 1
    }
}
