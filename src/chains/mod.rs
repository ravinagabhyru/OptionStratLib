//! # Chains Module
//!
//! This module provides functionality for working with option chains and their components.
//! It includes tools for building, managing, and manipulating option chains, as well as
//! handling multiple-leg option strategies.
//!
//! ## Core Components
//!
//! * `chain` - Implements core option chain functionality (`OptionChain` and `OptionData` structures)
//! * `legs` - Provides strategy leg combinations through the `StrategyLegs` enum
//! * `utils` - Contains utility functions and parameter structures for chain operations
//!
//! ## Main Features
//!
//! * Option chain construction and management
//! * Support for various option data formats
//! * Import/export capabilities (CSV, JSON)
//! * Multiple-leg strategy support
//! * Price calculation and volatility adjustments
//!
//! ## Example Usage
//!
//! ```rust
//! use optionstratlib::chains::chain::{OptionChain, OptionData};
//! use optionstratlib::chains::utils::{OptionChainBuildParams, OptionDataPriceParams};
//! use optionstratlib::{f2p, sf2p, ExpirationDate};
//! use optionstratlib::{spos, Positive};
//!
//! let option_chain_params = OptionChainBuildParams::new(
//!             "SP500".to_string(),
//!             None,
//!             10,
//!             f2p!(1.0),
//!             0.0,
//!             f2p!(0.02),
//!             2,
//!             OptionDataPriceParams::new(
//!                 f2p!(100.0),
//!                 ExpirationDate::Days(30.0),
//!                 sf2p!(0.17),
//!                 0.0,
//!                 0.05,
//!             ),
//!         );
//!
//! let built_chain = OptionChain::build_chain(&option_chain_params);
//! assert_eq!(built_chain.symbol, "SP500");
//! assert_eq!(built_chain.underlying_price, Positive::new(100.0).unwrap());
//! ```
//!
//! ## Strategy Legs Support
//!
//! The module supports various option strategy combinations through the `StrategyLegs` enum:
//!
//! * Two-leg strategies (e.g., spreads)
//! * Four-leg strategies (e.g., iron condors)
//! * Six-leg strategies (e.g., butterfly variations)
//!
//! ## Utility Functions
//!
//! The module provides various utility functions for:
//!
//! * Strike price generation
//! * Volatility adjustment
//! * Price calculations
//! * Data parsing and formatting
//!
//! ## File Handling
//!
//! Supports both CSV and JSON formats for:
//!
//! * Importing option chain data
//! * Exporting option chain data
//! * Maintaining consistent data formats
//!
//!
//!
//! # Risk Neutral Density (RND) Analysis Module
//!
//! This module implements functionality to calculate and analyze the Risk-Neutral Density (RND)
//! from option chains. The RND represents the market's implied probability distribution of
//! future asset prices and is a powerful tool for understanding market expectations.
//!
//! ## Theory and Background
//!
//! The Risk-Neutral Density (RND) is a probability distribution that represents the market's
//! view of possible future prices of an underlying asset, derived from option prices. It is
//! "risk-neutral" because it incorporates both the market's expectations and risk preferences
//! into a single distribution.
//!
//! Key aspects of RND:
//! - Extracted from option prices using the Breeden-Litzenberger formula
//! - Provides insights into market sentiment and expected volatility
//! - Used for pricing exotic derivatives and risk assessment
//!
//! ## Statistical Moments and Their Interpretation
//!
//! The module calculates four key statistical moments:
//!
//! 1. **Mean**: The expected future price of the underlying asset
//! 2. **Variance**: Measure of price dispersion, related to expected volatility
//! 3. **Skewness**: Indicates asymmetry in price expectations
//!    - Positive skew: Market expects upside potential
//!    - Negative skew: Market expects downside risks
//! 4. **Kurtosis**: Measures the likelihood of extreme events
//!    - High kurtosis: Market expects "fat tails" (more extreme moves)
//!    - Low kurtosis: Market expects more moderate price movements
//!
//! ## Usage Example
//!
//! ```rust
//! use rust_decimal_macros::dec;
//! use tracing::info;
//! use optionstratlib::chains::{RNDParameters, RNDAnalysis};
//! use optionstratlib::chains::chain::OptionChain;
//! use optionstratlib::chains::utils::{OptionChainBuildParams, OptionDataPriceParams};
//! use optionstratlib::{pos, ExpirationDate, Positive};
//!
//! // Create parameters for RND calculation
//! let params = RNDParameters {
//!     risk_free_rate: dec!(0.05),
//!     interpolation_points: 100,
//!     derivative_tolerance: pos!(0.001),
//! };
//! let chain = OptionDataPriceParams::new(
//!     Positive::new(2000.0).unwrap(),
//!     ExpirationDate::Days(10.0),
//!     Some(Positive::new(0.01).unwrap()),
//!     0.01,
//!     0.0,
//! );
//!
//! let option_chain_params = OptionChainBuildParams::new(
//!     "SP500".to_string(),
//!     Some(Positive::ONE),
//!     5,
//!     Positive::ONE,
//!     0.0001,
//!     Positive::new(0.02).unwrap(),
//!     2,
//!     chain,
//! );
//!
//! let option_chain = OptionChain::build_chain(&option_chain_params);
//! // Calculate RND from option chain
//! let rnd_result = option_chain.calculate_rnd(&params).unwrap();
//!
//! // Access statistical moments
//! info!("Expected price: {}", rnd_result.statistics.mean);
//! info!("Implied volatility: {}", rnd_result.statistics.volatility);
//! info!("Market bias: {}", rnd_result.statistics.skewness);
//! info!("Tail risk: {}", rnd_result.statistics.kurtosis);
//! ```
//!
//! ## Market Insights from RND
//!
//! The RND provides several valuable insights:
//!
//! 1. **Price Expectations**
//!    - Mean indicates the market's expected future price
//!    - Variance shows uncertainty around this expectation
//!
//! 2. **Market Sentiment**
//!    - Skewness reveals directional bias
//!    - Kurtosis indicates expected market stability
//!
//! 3. **Risk Assessment**
//!    - Shape of distribution helps quantify various risks
//!    - Particularly useful for stress testing and VaR calculations
//!
//! 4. **Volatility Structure**
//!    - Implied volatility skew analysis
//!    - Term structure of market expectations
//!
//! ## Mathematical Foundation
//!
//! The RND is calculated using the Breeden-Litzenberger formula:
//!
//! ```text
//! q(K) = e^(rT) * (∂²C/∂K²)
//! ```
//!
//! Where:
//! - q(K) is the RND value at strike K
//! - r is the risk-free rate
//! - T is time to expiration
//! - C is the call option price
//! - ∂²C/∂K² is the second derivative with respect to strike
//!
//! ## Implementation Details
//!
//! The module implements:
//! - Numerical approximation of derivatives
//! - Statistical moment calculations
//! - Error handling for numerical stability
//! - Volatility skew analysis
//!
//! The implementation focuses on numerical stability and accurate moment calculations,
//! particularly for extreme market conditions.
pub mod chain;
mod legs;
pub mod utils;

mod options;

mod rnd;

pub use legs::StrategyLegs;

pub use options::{DeltasInStrike, OptionsInStrike};

pub use rnd::{RNDAnalysis, RNDParameters, RNDResult};
