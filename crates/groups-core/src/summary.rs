//! Box-plot summaries for trend charts.

use crate::error::AnalysisError;

/// The five numbers of a box plot. The quartiles are Hyndman and Fan's
/// type 7, the default of R, NumPy and Excel's QUARTILE.INC: the value at
/// position p (n − 1) of the sorted values, counted from 0, interpolated
/// linearly between neighbours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxPlot {
    pub count: usize,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
}

impl BoxPlot {
    /// The box plot of `values`, which must all be finite.
    pub fn new(values: &[f64]) -> Result<BoxPlot, AnalysisError> {
        if values.is_empty() {
            return Err(AnalysisError::Empty);
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(AnalysisError::NotFinite);
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        Ok(BoxPlot {
            count: sorted.len(),
            min: sorted[0],
            q1: quantile(&sorted, 0.25),
            median: quantile(&sorted, 0.5),
            q3: quantile(&sorted, 0.75),
            max: sorted[sorted.len() - 1],
        })
    }
}

/// The type-7 quantile of sorted, non-empty `values`.
fn quantile(values: &[f64], p: f64) -> f64 {
    let position = p * (values.len() - 1) as f64;
    let below = position.floor() as usize;
    let above = (below + 1).min(values.len() - 1);
    let fraction = position - below as f64;
    if fraction == 0.0 {
        return values[below];
    }
    // As a weighted sum, so values near ±f64::MAX don't overflow.
    (1.0 - fraction) * values[below] + fraction * values[above]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quartiles_by_hand() {
        // R: quantile(c(1, 2, 4, 7, 11, 16), type = 7) gives 1, 2.5, 5.5,
        // 10, 16.
        let plot = BoxPlot::new(&[16.0, 1.0, 7.0, 2.0, 11.0, 4.0]).unwrap();
        assert_eq!(
            plot,
            BoxPlot {
                count: 6,
                min: 1.0,
                q1: 2.5,
                median: 5.5,
                q3: 10.0,
                max: 16.0
            }
        );
        // Five values: the quartiles are the 2nd and 4th.
        let plot = BoxPlot::new(&[5.0, 3.0, 1.0, 4.0, 2.0]).unwrap();
        assert_eq!((plot.q1, plot.median, plot.q3), (2.0, 3.0, 4.0));
        // One value, or all equal: every number is it.
        let one = BoxPlot::new(&[0.7]).unwrap();
        assert_eq!(
            (one.min, one.q1, one.median, one.q3, one.max),
            (0.7, 0.7, 0.7, 0.7, 0.7)
        );
        let same = BoxPlot::new(&[2.0; 4]).unwrap();
        assert_eq!((same.q1, same.q3), (2.0, 2.0));
        // Two values: quarter and three-quarter of the way.
        let two = BoxPlot::new(&[0.0, 4.0]).unwrap();
        assert_eq!((two.q1, two.median, two.q3), (1.0, 2.0, 3.0));
    }

    #[test]
    fn bad_inputs_are_refused() {
        assert_eq!(BoxPlot::new(&[]), Err(AnalysisError::Empty));
        assert_eq!(
            BoxPlot::new(&[1.0, f64::NAN]),
            Err(AnalysisError::NotFinite)
        );
        assert_eq!(
            BoxPlot::new(&[f64::INFINITY]),
            Err(AnalysisError::NotFinite)
        );
        // The largest finite values stay finite.
        let wide = BoxPlot::new(&[-1e308, 1e308, 1e308]).unwrap();
        assert_eq!((wide.q1, wide.median), (0.0, 1e308));
        let plot = BoxPlot::new(&[-f64::MAX, f64::MAX]).unwrap();
        assert!(
            [plot.q1, plot.median, plot.q3]
                .iter()
                .all(|q| q.is_finite())
        );
    }
}
