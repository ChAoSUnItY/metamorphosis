use std::{collections::HashMap, hash::Hash};

#[derive(Clone)]
pub struct Grouping<I, Ks, K>
where
    I: Iterator,
    Ks: FnMut(&I::Item) -> K,
{
    pub(crate) iter: I,
    key_selector: Ks,
}

impl<I, Ks, K> Grouping<I, Ks, K>
where
    I: Iterator,
    Ks: FnMut(&I::Item) -> K,
{
    pub(crate) fn new(iter: I, key_selector: Ks) -> Self {
        Self { iter, key_selector }
    }
}

impl<I, Ks, K> Iterator for Grouping<I, Ks, K>
where
    I: Iterator,
    Ks: FnMut(&I::Item) -> K,
{
    type Item = (K, I::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let Some(item) = self.iter.next() else {
            return None;
        };
        let key = (self.key_selector)(&item);

        Some((key, item))
    }
}

#[allow(dead_code)]
impl<I, Ks, K> Grouping<I, Ks, K>
where
    I: Iterator,
    Ks: FnMut(&I::Item) -> K,
    K: Eq + Hash,
{
    pub fn aggregate<R, O>(mut self, mut operation: O) -> HashMap<K, R>
    where
        O: FnMut(&K, Option<R>, I::Item) -> R,
    {
        let mut m = HashMap::new();

        while let Some((key, value)) = self.next() {
            if let Some(entry) = m.remove(&key) {
                let accumulator = operation(&key, Some(entry), value);

                m.insert(key, accumulator);
            } else {
                let value = operation(&key, None, value);

                m.insert(key, value);
            }
        }

        m
    }

    pub fn fold_with_key<R, Ivs, O>(
        self,
        mut initial_value_selector: Ivs,
        mut operation: O,
    ) -> HashMap<K, R>
    where
        Ivs: FnMut(&K, &I::Item) -> R,
        O: FnMut(&K, R, I::Item) -> R,
    {
        self.aggregate(|key, accumulator, item| {
            operation(
                key,
                accumulator.unwrap_or(initial_value_selector(key, &item)),
                item,
            )
        })
    }

    pub fn fold_with<R, Ivg, O>(
        self,
        mut initial_value_provider: Ivg,
        mut operation: O,
    ) -> HashMap<K, R>
    where
        Ivg: FnMut() -> R,
        O: FnMut(&K, R, I::Item) -> R,
    {
        self.aggregate(|key, accumulator, item| {
            operation(key, accumulator.unwrap_or(initial_value_provider()), item)
        })
    }

    pub fn fold<R, O>(self, initial_value: R, mut operation: O) -> HashMap<K, R>
    where
        O: FnMut(R, I::Item) -> R,
        R: Clone,
    {
        self.aggregate(|_, accumulator, item| {
            operation(accumulator.unwrap_or(initial_value.clone()), item)
        })
    }

    pub fn reduce_with_key<R, O>(self, mut operation: O) -> HashMap<K, R>
    where
        O: FnMut(&K, R, I::Item) -> R,
        I::Item: Into<R>,
    {
        self.aggregate(|key, accumulator, item| {
            if let Some(accumulator) = accumulator {
                operation(key, accumulator, item)
            } else {
                item.into()
            }
        })
    }

    pub fn reduce<R, O>(self, mut operation: O) -> HashMap<K, R>
    where
        O: FnMut(R, I::Item) -> R,
        I::Item: Into<R>,
    {
        self.reduce_with_key(|_, accumulator, item| operation(accumulator, item))
    }

    pub fn each_count(self) -> HashMap<K, usize> {
        self.fold(0, |accumulator, _| accumulator + 1)
    }
}

pub trait IntoGrouping<I>
where
    I: Iterator,
{
    fn grouping_by<Ks, K>(self, key_selector: Ks) -> Grouping<I, Ks, K>
    where
        Ks: FnMut(&I::Item) -> K;
}

impl<I> IntoGrouping<I> for I
where
    I: Iterator,
{
    fn grouping_by<Ks, K>(self, key_selector: Ks) -> Grouping<I, Ks, K>
    where
        Ks: FnMut(&I::Item) -> K,
    {
        Grouping::new(self, key_selector)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::IntoGrouping;

    #[test]
    fn test_grouping_iteration() {
        let mut values = (0..10).into_iter().grouping_by(|i| *i % 3);

        assert_eq!(values.next(), Some((0, 0)));
        assert_eq!(values.next(), Some((1, 1)));
        assert_eq!(values.next(), Some((2, 2)));
        assert_eq!(values.next(), Some((0, 3)));
        assert_eq!(values.next(), Some((1, 4)));
        assert_eq!(values.next(), Some((2, 5)));
        assert_eq!(values.next(), Some((0, 6)));
        assert_eq!(values.next(), Some((1, 7)));
        assert_eq!(values.next(), Some((2, 8)));
        assert_eq!(values.next(), Some((0, 9)));
        assert_eq!(values.next(), None);
    }

    #[test]
    fn test_grouping_aggregate() {
        let values = (3..=9).into_iter().grouping_by(|i| *i % 3);
        let aggregated = values.aggregate(|key, accumulator: Option<String>, item| {
            if let Some(mut accumulator) = accumulator {
                accumulator.push_str(&format!("-{}", item));
                accumulator
            } else {
                format!("{}:{}", key, item)
            }
        });

        assert_eq!(
            aggregated,
            HashMap::from([
                (0, "0:3-6-9".to_string()),
                (1, "1:4-7".to_string()),
                (2, "2:5-8".to_string())
            ])
        );
    }

    #[test]
    fn test_fold_with_key() {
        let fruits = vec![
            "cherry",
            "blueberry",
            "citrus",
            "apple",
            "apricot",
            "banana",
            "coconut",
        ]
        .into_iter()
        .grouping_by(|fruit_name| fruit_name.chars().next().unwrap());
        let even_fruits = fruits.fold_with_key(
            |_, _| vec![],
            |_, mut accumulator, item| {
                if item.len() % 2 == 0 {
                    accumulator.push(item.to_string());
                }

                accumulator
            },
        );

        assert_eq!(
            even_fruits,
            HashMap::from([
                ('a', vec![]),
                ('b', vec!["banana".to_string()]),
                ('c', vec!["cherry".to_string(), "citrus".to_string()])
            ])
        );
    }
}
