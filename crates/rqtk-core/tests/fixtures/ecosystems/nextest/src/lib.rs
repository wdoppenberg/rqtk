pub fn double(x: i32) -> i32 { x * 2 }

#[cfg(test)]
mod tests {
    use super::*;

    // rqtk: verifies VA-RS-01
    #[test]
    fn doubles() {
        assert_eq!(double(2), 4);
    }

    // rqtk: verifies VA-RS-02
    #[test]
    #[cfg_attr(
        not(test),
        ignore
    )]
    fn cut_off() {
        assert_eq!(double(0), 0);
    }

    mod nested {
        // rqtk: verifies VA-RS-03
        #[test]
        fn deep() {}
    }

    // rqtk: verifies VA-RS-04
    #[test]
    #[ignore]
    fn ignored() {}
}
