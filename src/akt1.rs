use std::{collections::HashMap, fmt::Display, hash::Hash};

pub trait Grouping<T, K>
where
    K: Hash,
    K: Eq,
{
    fn source_iterator<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        T: 'a;
    fn key_of(&self, element: &T) -> K;

    fn aggregate<R>(&self, operation: impl Fn(&K, Option<R>, &T) -> R) -> HashMap<K, R> {
        let mut m = HashMap::new();

        for item in self.source_iterator() {
            let key = self.key_of(item);
            let value = m.remove(&key).map_or_else(
                || operation(&key, None, item),
                |accumulator| operation(&key, Some(accumulator), item),
            );

            m.insert(key, value);
        }

        m
    }

    fn fold_with_key<R>(
        &self,
        initial_value_selector: impl Fn(&K, &T) -> R,
        operation: impl Fn(&K, R, &T) -> R,
    ) -> HashMap<K, R> {
        self.aggregate(|key, accumulator, item| {
            operation(
                key,
                accumulator.unwrap_or(initial_value_selector(key, item)),
                item,
            )
        })
    }

    fn fold<R>(&self, initial_value: R, operation: impl Fn(R, &T) -> R) -> HashMap<K, R>
    where
        R: Clone,
    {
        self.aggregate(|_, accumulator, item| {
            operation(accumulator.unwrap_or(initial_value.clone()), item)
        })
    }

    fn reduce_with_key<R>(&self, operation: impl Fn(&K, R, &T) -> R) -> HashMap<K, R>
    where
        T: Clone,
        T: Into<R>,
    {
        self.aggregate(|key, accumulator, item| {
            if let Some(accumulator) = accumulator {
                operation(key, accumulator, item)
            } else {
                item.clone().into()
            }
        })
    }

    fn each_count(&self) -> HashMap<K, usize> {
        self.fold(0, |accumulator, _| accumulator + 1)
    }
}

pub struct GroupingImpl<'ks, T, K>
where
    K: Eq,
    K: Hash,
{
    raw: Vec<T>,
    key_selector: Box<dyn Fn(&T) -> K + 'ks>,
}

impl<'ks, T, K> Grouping<T, K> for GroupingImpl<'ks, T, K>
where
    K: Eq,
    K: Hash,
{
    fn source_iterator<'src>(&'src self) -> impl Iterator<Item = &'src T>
    where
        T: 'src,
    {
        (&self.raw).into_iter()
    }

    fn key_of(&self, element: &T) -> K {
        self.key_selector.as_ref()(element)
    }
}

impl<'ks, T, K> Display for GroupingImpl<'ks, T, K>
where
    K: Eq,
    K: Hash,
    T: Display,
    K: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut m: HashMap<K, Vec<&T>> = HashMap::new();

        for src in self.source_iterator() {
            m.entry(self.key_of(src)).or_default().push(src);
        }

        let mut first = true;

        write!(f, "{{")?;

        for (k, v) in m.into_iter() {
            if !first {
                write!(f, ", ")?;
            } else {
                first = false;
            }

            write!(f, "{}=[", k)?;

            let mut inner_first = true;

            for item in v.into_iter() {
                if !inner_first {
                    write!(f, ", ")?;
                } else {
                    inner_first = false;
                }

                write!(f, "{}", item)?;
            }

            write!(f, "]")?;
        }

        write!(f, "}}")
    }
}

pub trait IntoGrouping<'ks, T, K>
where
    K: Eq,
    K: Hash,
{
    fn grouping_by(self, key_selector: impl Fn(&T) -> K + 'ks) -> GroupingImpl<'ks, T, K>;
}

impl<'a, 'ks, T, K> IntoGrouping<'ks, T, K> for Vec<T>
where
    K: Eq,
    K: Hash,
{
    fn grouping_by(self, key_selector: impl Fn(&T) -> K + 'ks) -> GroupingImpl<'ks, T, K> {
        GroupingImpl {
            raw: self,
            key_selector: Box::new(key_selector),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::{Grouping, IntoGrouping};

    #[test]
    fn test_each_count() {
        let words = "one two three four five six seven eight nine ten"
            .split(" ")
            .collect::<Vec<_>>();
        let freq_by_first_char = words
            .grouping_by(|s| s.chars().next().unwrap())
            .each_count();

        // {o=1, t=3, f=2, s=2, e=1, n=1}
        assert_eq!(
            freq_by_first_char,
            HashMap::from([('o', 1), ('t', 3), ('f', 2), ('s', 2), ('e', 1), ('n', 1)])
        )
    }

    #[test]
    fn test_aggregate() {
        let numbers = (3..=9).collect::<Vec<usize>>();
        let aggregated =
            numbers
                .grouping_by(|i| *i % 3)
                .aggregate(|key, accumulator: Option<String>, item| {
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
        ];
        let even_fruits = fruits
            .grouping_by(|f| f.chars().next().unwrap())
            .fold_with_key(
                |&k, _| (k, vec![]),
                |_, mut accumulator, item| {
                    if item.len() % 2 == 0 {
                        accumulator.1.push(item.to_string());
                    }

                    accumulator
                },
            );

        assert_eq!(
            even_fruits,
            HashMap::from([
                ('a', ('a', vec![])),
                ('b', ('b', vec!["banana".to_string()])),
                ('c', ('c', vec!["cherry".to_string(), "citrus".to_string()]))
            ])
        );
    }

    #[test]
    fn test_fold() {
        let fruits = vec![
            "apple",
            "apricot",
            "banana",
            "blueberry",
            "cherry",
            "coconut",
        ];
        let even_fruits = fruits.grouping_by(|f| f.chars().next().unwrap()).fold(
            vec![],
            |mut accumulator, item| {
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
                ('c', vec!["cherry".to_string()])
            ])
        );
    }

    #[test]
    fn test_reduce() {
        let animals = vec!["raccoon", "reindeer", "cow", "camel", "giraffe", "goat"];
        let is_vowel = |&c: &char| c == 'a' || c == 'e' || c == 'i' || c == 'o' || c == 'u';
        let max_vowels = animals
            .grouping_by(|a| a.chars().next().unwrap())
            .reduce_with_key(|_, accumulator: &str, item| {
                let acc_vowels = accumulator.chars().filter(is_vowel).count();
                let item_vowels = item.chars().filter(is_vowel).count();

                match acc_vowels.cmp(&item_vowels) {
                    std::cmp::Ordering::Less => item,
                    std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => &accumulator,
                }
            });

        assert_eq!(
            max_vowels,
            HashMap::from([('r', "reindeer"), ('c', "camel"), ('g', "giraffe")])
        );
    }
}
