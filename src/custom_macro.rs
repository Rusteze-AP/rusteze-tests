#[macro_export]
macro_rules! assert_matches_any {
    ($res:expr, $expected1:expr, $expected2:expr) => {
        assert!(
            $res == $expected1 || $res == $expected2,
            "Assertion failed: `res` does not match any expected values.\n\
             Actual: `{:?}`\n\
             Expected: `{:?}`\nOR\n`{:?}`",
            $res,
            $expected1,
            $expected2
        );
    };
}
