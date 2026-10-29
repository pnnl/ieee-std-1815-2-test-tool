/// A multiplexed family and its 1-based entry number.
///
/// The point type and point index are selected separately within this entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiplexerNumber {
    Curve(u16),
    ScheduleBC(u16),
    Schedule(u16),
}
