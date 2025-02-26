//! # Random Walk Module
//!
//! This module implements random walk simulations for financial asset price modeling,
//! with support for varying volatility and trend components. It provides tools for
//! generating and analyzing price paths with customizable parameters.
//!
//! ## Core Components
//!
//! ### Walkable Trait
//!
//! ```rust
//! use optionstratlib::Positive;
//!
//! pub trait Walkable {
//!     fn get_y_values(&mut self) -> &mut Vec<Positive>;
//!     fn generate_random_walk(
//!         &mut self,
//!         n_steps: usize,
//!         initial_price: Positive,
//!         mean: f64,
//!         std_dev: Positive,
//!         std_dev_change: Positive,
//!     );
//! }
//! ```
//!
//! ### RandomWalkGraph
//!
//! A structure that implements both the Walkable trait and price path visualization:
//!
//! ```rust
//! use optionstratlib::Positive;
//!
//! use optionstratlib::utils::time::TimeFrame;
//!
//! pub struct RandomWalkGraph {
//!     values: Vec<Positive>,
//!     title_text: String,
//!     current_index: usize,
//!     risk_free_rate: Option<f64>,
//!     dividend_yield: Option<f64>,
//!     time_frame: TimeFrame,
//!     volatility_window: usize,
//!     initial_volatility: Option<Positive>,
//! }
//! ```
//!
//! ## Usage Examples
//!
//! ### Basic Random Walk Generation
//!
//! ```rust
//! use rust_decimal_macros::dec;
//! use optionstratlib::{spos, Positive};
//! use optionstratlib::utils::time::TimeFrame;
//! use optionstratlib::pos;
//! use optionstratlib::simulation::{RandomWalkGraph, Walkable};
//!
//! // Create a new random walk graph
//! let mut walk = RandomWalkGraph::new(
//!     "Asset Price Simulation".to_string(),
//!     Some(dec!(0.05)),   // risk-free rate
//!     spos!(0.02),   // dividend yield
//!     TimeFrame::Day,
//!     20,   // volatility window
//!     Some(pos!(0.2)) // initial volatility
//! );
//!
//! // Generate the random walk
//! walk.generate_random_walk(
//!     252,   // number of steps (1 year of trading days)
//!     pos!(100.0),   // initial price
//!     0.0,   // mean (drift)
//!     pos!(0.2),   // standard deviation
//!     pos!(0.01)  // volatility of volatility
//! );
//! ```
//!
//! ### Using the Iterator Interface
//!
//! ```rust
//! use rust_decimal_macros::dec;
//! use tracing::info;
//! use optionstratlib::Positive;
//! use optionstratlib::utils::time::TimeFrame;
//! use optionstratlib::pos;
//! use optionstratlib::simulation::{RandomWalkGraph, Walkable};
//!
//! let mut walk = RandomWalkGraph::new(
//!     "Price Path".to_string(),
//!     Some(dec!(0.05)),
//!     Some(pos!(0.02)),
//!     TimeFrame::Day,
//!     20,
//!     Some(pos!(0.2))
//! );
//!
//! // Generate path
//! walk.generate_random_walk(
//!     252,
//!     pos!(100.0),
//!     0.0,
//!     pos!(0.2),
//!     pos!(0.01)
//! );
//!
//! // Iterate through the price path
//! for params in &mut walk {
//!     info!(
//!         "Price: {}, Volatility: {:?}",
//!         params.get_underlying_price(),
//!         params.get_implied_volatility()
//!     );
//! }
//! ```
//!
//! ### Visualization Integration
//!
//! ```rust
//! use rust_decimal_macros::dec;
//! use optionstratlib::visualization::utils::Graph;
//! use optionstratlib::{spos, Positive};
//! use optionstratlib::pos;
//! use optionstratlib::simulation::RandomWalkGraph;
//! use optionstratlib::utils::time::TimeFrame;
//!
//! let mut walk = RandomWalkGraph::new(
//!     "Price Path".to_string(),
//!     Some(dec!(0.05)),
//!     spos!(0.02),
//!     TimeFrame::Day,
//!     20,
//!     Some(pos!(0.2))
//! );
//!
//! // Get values for plotting
//! let values: Vec<f64> = walk.get_values(&[]);
//! ```
//!
//! ## Mathematical Background
//!
//! The random walk implementation follows the geometric Brownian motion model with:
//!
//! 1. Price changes: dS = μSdt + σSdW
//!    - S: Asset price
//!    - μ: Drift (mean return)
//!    - σ: Volatility
//!    - dW: Wiener process increment
//!
//! 2. Volatility updates: σ(t) ~ N(σ, σ_change)
//!    - Stochastic volatility component
//!    - Updates based on volatility_window
//!
//! ## Features
//!
//! - Geometric Brownian motion simulation
//! - Stochastic volatility modeling
//! - Real-time volatility estimation
//! - Integration with option pricing parameters
//! - Visualization support
//! - Iterator interface for sequential processing
//!
//! ## Performance Considerations
//!
//! - Time Complexity: O(n) for generation, where n is the number of steps
//! - Space Complexity: O(n) for storing the price path
//! - Volatility calculation: O(w) where w is the volatility window size
//!
//! ## Implementation Notes
//!
//! - All prices are strictly positive (enforced by Positive)
//! - Volatility is estimated using rolling windows
//! - The iterator provides option pricing parameters for each step
//! - Thread-safe random number generation
//! - Supports various time frames (daily, weekly, monthly)

mod model;
mod simulator;
pub mod walk;

pub use model::SimulationResult;
pub use simulator::{SimulationConfig, Simulator, WalkId};
pub use walk::{RandomWalkGraph, Walkable};
