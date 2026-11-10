pub(crate) mod store;
pub mod internal {
    pub mod api;
    pub mod connection;
    pub mod exec;
    pub mod launcher;
    pub mod paths;
    pub mod sudo;
    #[cfg(test)]
    pub(crate) mod test_support;
    pub mod transfer;
    pub mod types;
}
