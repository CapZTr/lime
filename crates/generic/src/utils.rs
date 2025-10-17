use crate::cost::Cost;

pub trait Mean {
    type T;

    fn mean(self) -> Self::T;
}

impl<I: IntoIterator<Item = Cost>> Mean for I {
    type T = Option<Cost>;

    fn mean(self) -> Self::T {
        let mut sum = Cost::from(0.0);
        let mut n = 0;
        for v in self {
            n += 1;
            sum += v;
        }
        if n == 0 {
            None
        } else {
            Some(sum / Cost::from(n))
        }
    }
}
