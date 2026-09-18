#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TokenSide {
    X,
    Y,
}

impl From<bool> for TokenSide {
    fn from(is_x: bool) -> Self {
        if is_x {
            Self::X
        } else {
            Self::Y
        }
    }
}

impl TokenSide {
    pub(crate) fn opposite(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::X,
        }
    }
}

pub(crate) fn select_for_side<T>(side: TokenSide, x: T, y: T) -> T {
    match side {
        TokenSide::X => x,
        TokenSide::Y => y,
    }
}

#[cfg(test)]
mod tests {
    use super::{select_for_side, TokenSide};

    #[test]
    fn converts_bool_to_token_side() {
        assert_eq!(TokenSide::from(true), TokenSide::X);
        assert_eq!(TokenSide::from(false), TokenSide::Y);
    }

    #[test]
    fn returns_the_opposite_token_side() {
        assert_eq!(TokenSide::X.opposite(), TokenSide::Y);
        assert_eq!(TokenSide::Y.opposite(), TokenSide::X);
    }

    #[test]
    fn selects_the_value_for_the_requested_side() {
        assert_eq!(select_for_side(TokenSide::X, "x", "y"), "x");
        assert_eq!(select_for_side(TokenSide::Y, "x", "y"), "y");
    }
}
