//! Calibration: turns a method's raw score into a confidence that means
//! what it says (0.9: right about 9 times in 10), learned from the method's
//! own guesses on past data.
//!
//! Isotonic regression (pool adjacent violators): the confidence never goes
//! down as the score goes up. Each pooled block's rate is smoothed toward
//! one half (one right and one wrong guess added), so scant evidence never
//! reaches [`suggest::CONFIDENT`].

/// A score's confidence, by score range.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Curve {
    /// `(lowest score, confidence per mille)`, by score, confidence
    /// nondecreasing.
    steps: Vec<(f32, u16)>,
}

impl Curve {
    /// From `(score, right)` pairs.
    #[must_use]
    pub fn fit(mut points: Vec<(f32, bool)>) -> Self {
        points.retain(|p| p.0.is_finite());
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        // (lowest score, right, guesses)
        let mut blocks: Vec<(f32, f32, f32)> = Vec::new();
        for (score, right) in points {
            let mut b = (score, if right { 1.0 } else { 0.0 }, 1.0);
            if let Some(last) = blocks.last_mut()
                && last.0.total_cmp(&score).is_eq()
            {
                last.1 += b.1;
                last.2 += 1.0;
            } else {
                blocks.push(b);
            }
            while blocks.len() > 1 {
                let n = blocks.len();
                let (prev, cur) = (blocks[n - 2], blocks[n - 1]);
                if prev.1 / prev.2 < cur.1 / cur.2 {
                    break;
                }
                b = (prev.0, prev.1 + cur.1, prev.2 + cur.2);
                blocks.truncate(n - 2);
                blocks.push(b);
            }
        }
        let mut steps: Vec<(f32, u16)> = Vec::with_capacity(blocks.len());
        let mut floor = 0;
        for (low, right, n) in blocks {
            let rate = (right + 1.0) / (n + 2.0);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a rate in [0, 1] times 1000"
            )]
            let c = ((rate * 1000.0).round() as u16).max(floor);
            floor = c;
            steps.push((low, c));
        }
        Self { steps }
    }

    /// The confidence, per mille, for a score: that of the highest range
    /// it reaches. Without data, a score's own value, capped well below
    /// confident.
    #[must_use]
    pub fn apply(&self, score: f32) -> u16 {
        if self.steps.is_empty() {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped to [0, 500]"
            )]
            return (score.clamp(0.0, 1.0) * crate::params::UNCALIBRATED).round() as u16;
        }
        self.steps
            .iter()
            .take_while(|s| s.0 <= score)
            .last()
            .map_or(self.steps[0].1, |s| s.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_rises_with_the_score_and_needs_evidence() {
        let mut points = Vec::new();
        // Low scores: right half the time; high scores: always right.
        for i in 0..100 {
            points.push((0.2, i % 2 == 0));
            points.push((0.9, true));
        }
        // A violator: a mid score worse than the low ones.
        points.push((0.5, false));
        let c = Curve::fit(points);
        assert!(c.apply(0.2) <= 500, "{c:?}");
        assert_eq!(c.apply(0.95), 990);
        assert_eq!(c.apply(0.0), c.apply(0.2));
        assert!(c.apply(0.5) <= c.apply(0.9));
        // One right guess is not enough to be confident.
        assert!(Curve::fit(vec![(1.0, true)]).apply(1.0) < 900);
        assert_eq!(Curve::default().apply(1.0), 500);
    }
}
