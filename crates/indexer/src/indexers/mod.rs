pub mod hunter;
/// Indexer components: Scout, Hunter, and Shadow
///
/// This module contains the three main indexer components:
/// - Scout: Discovers new token launches on launchpads
/// - Hunter: Analyzes tokens to identify profitable early buyers
/// - Shadow: Tracks whale wallets for real-time purchase detection
pub mod scout;
pub mod shadow;

// Re-export indexer components
pub use hunter::Hunter;
pub use scout::Scout;
pub use shadow::Shadow;
