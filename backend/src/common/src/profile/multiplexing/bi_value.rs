#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiValue {
    /// The DNP3 BI point index.
    pub index: u16,
    /// The transmitted value for this point.
    pub value: bool,
}
