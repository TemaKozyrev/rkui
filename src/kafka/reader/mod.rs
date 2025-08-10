pub mod sequential;
pub mod merge;
pub mod merge_newest;
pub mod merge_oldest;

pub use sequential::consume_sequential;
pub use merge::consume_merge;
