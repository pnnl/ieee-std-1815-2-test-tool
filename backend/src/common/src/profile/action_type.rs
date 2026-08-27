#[repr(u8)] // Specifies the underlying type as u8
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Null = 0,
    AO = 1,
    BO = 2,
    Stop = 3,
}

impl TryFrom<u8> for ActionType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ActionType::Null),
            1 => Ok(ActionType::AO),
            2 => Ok(ActionType::BO),
            3 => Ok(ActionType::Stop),
            _ => Err("Invalid ActionType value"),
        }
    }
}
