use std::collections::HashMap;

use ordered_float::OrderedFloat;

pub mod criterion;
pub mod decision_tree;
pub mod node;
pub mod random_forest;
pub mod table;

fn mean(xs: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0;
    let mut total = 0.0;

    for x in xs {
        count += 1;
        total += x;
    }

    assert_ne!(count, 0);

    total / count as f64
}

fn most_frequent(xs: impl Iterator<Item = f64>) -> f64 {
    let (histogram, _) = histogram(xs);

    histogram
        .into_iter()
        .max_by_key(|t| t.1)
        .map(|t| (t.0).0)
        .unwrap()
}

fn histogram(xs: impl Iterator<Item = f64>) -> (HashMap<OrderedFloat<f64>, usize>, usize) {
    let mut histogram = HashMap::<_, usize>::new();
    let mut n = 0;

    for x in xs {
        *histogram.entry(OrderedFloat(x)).or_default() += 1;
        n += 1;
    }

    (histogram, n)
}
