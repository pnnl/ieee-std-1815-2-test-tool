/// A profile point with a numeric index.
pub trait Point {
    fn point_index(&self) -> u16;
}
